//! Slash commands and the composer autocomplete popup (commands,
//! agents, files, skills, emoji).

use crate::app::*;
use crate::*;

// ---- slash commands (codex-style) ----

pub(crate) struct Cmd {
    pub(crate) name: &'static str,
    /// what it does, then its usage after `: ` when it takes arguments
    pub(crate) desc: &'static str,
    /// its arguments in order, each with what completes it (BISE-117):
    /// the `/` popup offers them after the command's name
    pub(crate) args: &'static [Arg],
}

/// What completes one argument of a command (BISE-117).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Arg {
    /// one of these words, and what each does
    Words(&'static [(&'static str, &'static str)]),
    /// a live agent (not main)
    Task,
    /// an archived agent
    Archived,
    /// an open card, by its number
    Card,
    /// a version of switchboard (the hub's list: the commits, tree,
    /// back), after these words
    Version(&'static [(&'static str, &'static str)]),
    /// the same, the versions only in bise's source tree (`/restart`:
    /// elsewhere it only reloads, a commit is refused)
    DevVersion(&'static [(&'static str, &'static str)]),
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
    /// free text, required: the rest of the line (nothing to complete)
    Text,
    /// free text, optional: the value before it can already run
    Note,
}

const THEMES: &[(&str, &str)] =
    &[("auto", "your terminal's background"), ("light", "the light palette"), ("dark", "the dark palette")];

/// The slash commands the popup offers and `/help` lists.
pub(crate) const COMMANDS: &[Cmd] = &[
    // voice-menu (designer): one row, one ⏎, one screen; `/voice setup`
    // typed still opens it, on speech to text, but the menu offers no
    // argument
    Cmd { name: "/voice", desc: "dictation and voice mode: the model, the voice, the language", args: &[] },
    Cmd {
        name: "/restart",
        desc: "reload bise, nothing lost (a <commit>: bise's own sources only, built then switched): /restart [current|<commit>]",
        args: &[Arg::DevVersion(&[("current", "the version running now, nothing built")])],
    },
    Cmd { name: "/update", desc: "look for a new bise release now, and install it from its item (bise's source tree: build HEAD and restart on it)", args: &[] },
    Cmd {
        name: "/version",
        desc: "switchboard versions: /version [<commit>|tree|back]",
        // the hub's list holds tree and back
        args: &[Arg::Version(&[])],
    },
    Cmd {
        name: "/new",
        desc: "start an agent: /new [-w] [name:] objective",
        args: &[Arg::Words(&[("-w", "in its own git worktree")]), Arg::Text],
    },
    Cmd {
        name: "/archive",
        desc: "stop an agent and archive it, with its worktree: /archive <agent>",
        args: &[Arg::Task],
    },
    Cmd { name: "/restore", desc: "bring an archived agent back: /restore <agent>", args: &[Arg::Archived] },
    Cmd { name: "/archived", desc: "show or hide the archived agents in the panel", args: &[] },
    Cmd { name: "/isolate", desc: "give an agent its own git worktree: /isolate <agent>", args: &[Arg::Task] },
    Cmd { name: "/rename", desc: "rename an agent: /rename <agent> <new-name>", args: &[Arg::Task, Arg::Text] },
    Cmd { name: "/answer", desc: "answer an inbox item: /answer N text", args: &[Arg::Card, Arg::Text] },
    Cmd { name: "/close", desc: "close an inbox item without answering: /close N [note]", args: &[Arg::Card, Arg::Note] },
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
    },
    Cmd { name: "/artifacts", desc: "what your agents made: pages, docs, files, links", args: &[] },
    Cmd { name: "/scheduled", desc: "your scheduled tasks: list, open, run now, stop", args: &[] },
    Cmd { name: "/diff", desc: "a branch's changes against main, in a panel on the right: /diff [<branch>]", args: &[Arg::Branch] },
    Cmd { name: "/inbox", desc: "open what waits for you, the most blocking first (also /cards)", args: &[] },
    Cmd { name: "/agents", desc: "list the agents and what they do", args: &[] },
    Cmd { name: "/switch", desc: "find an agent by name, archived ones too, and open it (also cmd+k, ctrl+s)", args: &[] },
    Cmd {
        name: "/model",
        desc: "the model of the agent in view: /model [<model>] [default]",
        args: &[
            Arg::Model,
            Arg::Words(&[("default", "also for new sessions (config.toml [roles]: main, or agents for an agent)")]),
        ],
    },
    Cmd { name: "/models", desc: "which model does what: main, agents, small jobs (titles, summaries), voice", args: &[] },
    Cmd { name: "/provider", desc: "set up a provider's key, or change it", args: &[] },
    Cmd { name: "/reasoning", desc: "its reasoning effort: /reasoning [<effort>]", args: &[Arg::Effort] },
    Cmd { name: "/interrupt", desc: "interrupt the turn of the agent in view", args: &[] },
    Cmd {
        name: "/stop",
        desc: "stop an agent's turn and its hands on Chrome or an app until you write to it: /stop <agent>",
        args: &[Arg::Task],
    },
    Cmd {
        name: "/computer-use",
        desc: "turn on and set up computer use: agents drive Chrome and your apps, step by step: /computer-use [off|uninstall]",
        args: &[Arg::ComputerUse],
    },
    Cmd { name: "/compact", desc: "compact the conversation of the agent in view", args: &[] },
    Cmd {
        name: "/theme",
        desc: "light, dark, or auto (your terminal's background): /theme [auto|light|dark]",
        args: &[Arg::Words(THEMES)],
    },
    Cmd { name: "/welcome", desc: "replay the welcome of the first launch", args: &[] },
    Cmd { name: "/setup", desc: "check your terminal and repo again, and offer what would help", args: &[] },
    Cmd { name: "/help", desc: "the commands and the essential keys", args: &[] },
    Cmd { name: "/shortcuts", desc: "every keyboard shortcut (also /keys)", args: &[] },
    Cmd { name: "/quit", desc: "quit (the agents keep running)", args: &[] },
];

pub(crate) fn popup_matches(input: &str) -> Vec<&'static Cmd> {
    if !input.starts_with('/') || input.contains(' ') {
        return Vec::new();
    }
    COMMANDS.iter().filter(|c| c.name.starts_with(input)).collect()
}

/// One entry of the composer popup: a slash command, or an agent name
/// after `@`.
pub(crate) struct PopItem {
    pub(crate) label: String,
    pub(crate) desc: String,
    /// status glyph (mentions)
    pub(crate) mark: Option<(&'static str, Color)>,
    /// the composer text once picked with Tab (or Enter when `run` is
    /// None), and the cursor in it
    pub(crate) fill: String,
    pub(crate) fill_cursor: usize,
    /// Enter runs this line directly (commands without arguments)
    pub(crate) run: Option<String>,
    /// Esc closes the list and keeps the text (`@` and `$`); the slash
    /// popup clears the draft instead
    pub(crate) closable: bool,
    /// a workspace path: remembered when picked (ranked first next time)
    pub(crate) path: Option<String>,
    /// a folder of the `@` popup: `fill` browses it (`@path/`, the popup
    /// stays open on its entries); Tab, Enter and → take it
    pub(crate) folder: bool,
}

pub(crate) fn popup_items(app: &App) -> Vec<PopItem> {
    let mut cmds = popup_matches(&app.ed.text);
    // the dev build's own (BISE-235): nowhere else
    if app.ed.text.starts_with('/') && !app.ed.text.contains(' ') {
        cmds.extend(sb::release::dev_commands(app).iter().filter(|c| c.name.starts_with(app.ed.text.as_str())));
    }
    if !cmds.is_empty() {
        return cmds
            .into_iter()
            .map(|c| PopItem {
                label: c.name.to_string(),
                desc: c.desc.to_string(),
                mark: None,
                fill: format!("{} ", c.name),
                fill_cursor: c.name.chars().count() + 1,
                run: c.args.is_empty().then(|| c.name.to_string()),
                closable: false,
                path: None,
                folder: false,
            })
            .collect();
    }
    let args = arg_items(app);
    if !args.is_empty() {
        return args;
    }
    let at = at_items(app);
    if !at.is_empty() {
        return at;
    }
    let skills = skill_items(app);
    if skills.is_empty() {
        emoji_items(app)
    } else {
        skills
    }
}

/// One value an argument can take, as the popup shows it. An empty
/// `value` is a note that picks nothing ("loading the versions").
pub(crate) struct Choice {
    pub(crate) value: String,
    pub(crate) label: String,
    pub(crate) desc: String,
    pub(crate) mark: Option<(&'static str, Color)>,
}

impl Choice {
    fn word(value: &str, desc: &str) -> Choice {
        Choice { value: value.into(), label: value.into(), desc: desc.into(), mark: None }
    }
}

/// `q` is in one of `fields` (case-insensitive); an empty `q` matches.
pub(crate) fn matches(q: &str, fields: &[&str]) -> bool {
    let q = q.to_lowercase();
    q.is_empty() || fields.iter().any(|f| f.to_lowercase().contains(&q))
}

/// The argument being typed after a command's name: the command, the
/// line before the word being typed, the argument's index and that word.
/// None past a text argument (it takes the rest of the line).
pub(crate) fn arg_slot(text: &str) -> Option<(&'static Cmd, &str, usize, &str)> {
    if text.contains('\n') {
        return None;
    }
    let (name, rest) = text.split_once(' ')?;
    let cmd = COMMANDS.iter().chain(sb::release::DEV_COMMANDS).find(|c| c.name == name)?;
    let idx = rest.split(' ').count() - 1;
    let word = rest.rsplit(' ').next().unwrap_or("");
    if cmd.args.iter().take(idx).any(|a| matches!(a, Arg::Text | Arg::Note)) {
        return None;
    }
    Some((cmd, &text[..text.len() - word.len()], idx, word))
}

/// The values of `arg` matching `q`.
fn choices(app: &App, arg: Arg, q: &str) -> Vec<Choice> {
    let words = |ws: &[(&str, &str)]| -> Vec<Choice> {
        ws.iter().filter(|(w, _)| matches(q, &[w])).map(|(w, d)| Choice::word(w, d)).collect()
    };
    match arg {
        Arg::Words(ws) => words(ws),
        Arg::Version(ws) => {
            // tui-parity m_13350: a subcommand word is that subcommand,
            // never a filter: '/version list' ⏎ ran the top version whose
            // subject held 'list', a switch
            if let Some(c) = version_word(q) {
                return vec![c];
            }
            let mut out = words(ws);
            out.extend(sb::version_choices(app, q));
            out
        }
        Arg::DevVersion(ws) => {
            let mut out = words(ws);
            // asked in any workspace: the hub's answer says whether it is dev
            let vs = sb::version_choices(app, q);
            if sb::versions_dev(app) != Some(false) {
                out.extend(vs);
            }
            out
        }
        Arg::Task => sb::agent_choices(app, false, q),
        Arg::Archived => sb::agent_choices(app, true, q),
        Arg::Card => sb::card_choices(app, q),
        Arg::Branch => {
            let now = crate::when::now_ms();
            crate::diffbranches::branches(app)
                .into_iter()
                .filter(|b| matches(q, &[&b.branch]))
                .map(|b| Choice {
                    value: b.branch.clone(),
                    label: b.branch.clone(),
                    desc: format!("{}   {}", crate::diffbranches::branch_words(&b, now), crate::diffview::counts(b.add, b.del)),
                    mark: None,
                })
                .collect()
        }
        Arg::Plugin => crate::plugins::choices(std::path::Path::new(&sb::workspace(app).unwrap_or_default()), q, &app.ed.text),
        Arg::Model => model_choices(app, q),
        Arg::Effort => effort_choices(app, q),
        Arg::ComputerUse => computer_use_choices(crate::computer_use::is_on(), q),
        Arg::Text | Arg::Note => Vec::new(),
    }
}

/// `/version <q>` where `q` is a subcommand word, as its one row: `list`,
/// `back` and `rollback` (bise_proto::slash::version reads them) run
/// `/version <q>`; `restart` and `update` run `/restart` and `/update`.
/// None for any other word (a version to filter by).
fn version_word(q: &str) -> Option<Choice> {
    use bise_proto::slash::{version, Version};
    match q.trim().to_lowercase().as_str() {
        "restart" => Some(Choice::word("/restart", "reload bise, nothing lost")),
        "update" => Some(Choice::word("/update", "look for a new bise release now")),
        "" => None, // the bare '/version ': every version is offered
        w => match version(&format!("/version {w}"))? {
            Version::List => Some(Choice::word(w, "the installed versions")),
            Version::Rollback => Some(Choice::word(w, "back to the version before")),
            _ => None,
        },
    }
}

/// `/computer-use`'s rows follow the plugin: off, one row that turns it
/// back on (⏎ runs the bare `/computer-use`); on, the setup first (the
/// bare one), then off and uninstall. Never "off" when it is off (the
/// user's bug: off, then only off/uninstall were offered and ⏎ on the
/// bare command picked "off" again).
fn computer_use_choices(on: bool, q: &str) -> Vec<Choice> {
    let bare = |label: &str, desc: &str| Choice { value: "/computer-use".into(), label: label.into(), desc: desc.into(), mark: None };
    let rows = if on {
        vec![
            bare("setup", "the setup steps: browsers, extension, apps"),
            Choice::word("off", "turn computer use off"),
            Choice::word("uninstall", "turn it off and remove what it installed"),
        ]
    } else {
        vec![bare("on", "turn computer use on and set it up")]
    };
    rows.into_iter().filter(|c| matches(q, &[&c.label])).collect()
}

/// The note that heads `/model` and `/reasoning`: which agent they are
/// for (not global), and when it takes effect.
fn for_note(what: &str, agent: &str) -> Choice {
    Choice {
        value: String::new(),
        label: format!("{} for {}", what, agent),
        desc: "applies from its next call · esc cancels".into(),
        mark: None,
    }
}

/// `/model`: the catalog's chat models (built in and config.toml's)
/// under their provider's name (BISE-301: a header row that picks
/// nothing), then its aliases; the one the agent in view runs is marked
/// ✓. A
/// typed id the list does not have is offered last, as is (BISE-289):
/// `+ use <provider>/<id>` (no provider typed: the current model's).
fn model_choices(app: &App, q: &str) -> Vec<Choice> {
    let (agent, current, _, _) = sb::viewed_model(app);
    let mut out = vec![for_note("model", &agent)];
    let picks = crate::models::picks();
    let free = crate::models::free_id(q, current.split_once('/').map(|(p, _)| p).unwrap_or(""));
    let listed = |id: &str| picks.iter().any(|p| p.value == id);
    let mut group: Option<&str> = None;
    let shown: Vec<&crate::models::Pick> = picks.iter().filter(|p| matches(q, &[&p.value, &p.desc])).collect();
    // the id without its provider (the header says it)
    let id_of = |p: &crate::models::Pick| -> String {
        match p.value.split_once('/') {
            Some((_, id)) if !p.provider.is_empty() => id.to_string(),
            _ => p.value.clone(),
        }
    };
    // the descriptions in one column (designer's mock)
    let col = shown.iter().map(|p| unicode_width::UnicodeWidthStr::width(id_of(p).as_str())).max().unwrap_or(0) + 2;
    for p in shown {
        let head = if p.provider.is_empty() { "aliases" } else { p.provider.as_str() };
        if group != Some(head) {
            out.push(Choice { value: String::new(), label: head.into(), desc: String::new(), mark: None });
            group = Some(head);
        }
        let on = p.value == current;
        // the ✓ takes the indent's place, so the ids line up
        let id = id_of(p);
        let id = format!("{}{}", id, " ".repeat(col - unicode_width::UnicodeWidthStr::width(id.as_str())));
        out.push(Choice {
            label: if on { id } else { format!("  {}", id) },
            value: p.value.clone(),
            desc: p.short.clone(),
            mark: on.then(|| ("✓", theme::accent())),
        });
    }
    if let Some(id) = free.filter(|id| !listed(id) && !listed(q.trim())) {
        if out.len() == 1 {
            out.push(Choice { value: String::new(), label: "no listed model matches.".into(), desc: String::new(), mark: None });
        }
        // BISE-294: a provider with no key yet is set up first (/provider)
        out.push(match crate::models::keyless(&id) {
            Some((_, name)) => Choice {
                label: format!("+ set up {} for {}", name, id),
                value: id,
                desc: "no key yet: i'll ask for one".into(),
                mark: None,
            },
            None => Choice {
                label: format!("+ use {}", id),
                value: id,
                desc: "not in my list: its provider decides at the next call".into(),
                mark: None,
            },
        });
    }
    // BISE-294: the models listed are those of the providers set up; the
    // others are one row away
    let names = crate::models::not_ready_names();
    if q.trim().is_empty() || matches(q, &["another provider"]) {
        let desc = match names.len() {
            0 => "your keys, their models".to_string(),
            1..=3 => names.join(", "),
            _ => format!("{}…", names[..3].join(", ")),
        };
        out.push(Choice { value: "/provider".into(), label: "+ another provider…".into(), desc, mark: None });
    }
    // BISE-298: every role's model, one row away
    if q.trim().is_empty() || matches(q, &["every role", "roles"]) {
        out.push(Choice {
            value: "/models".into(),
            label: "every role…".into(),
            desc: "main, agents, small jobs, voice".into(),
            mark: None,
        });
    }
    out
}

/// What each effort does, on its row of `/reasoning`.
pub(crate) fn effort_hint(e: &str) -> &'static str {
    match e {
        "none" => "no reasoning, the fastest",
        "minimal" => "barely any reasoning",
        "low" => "faster, cheaper",
        "medium" => "balanced",
        "high" => "deeper, slower",
        "max" | "xhigh" => "the hard stuff",
        _ => "",
    }
}

/// `/reasoning`: the efforts the model of the agent in view takes, its
/// default and the current one marked; a model with none: one note.
fn effort_choices(app: &App, q: &str) -> Vec<Choice> {
    let (agent, model, current, efforts) = sb::viewed_model(app);
    let mut out = vec![for_note("reasoning", &agent)];
    if efforts.is_empty() {
        out.push(Choice {
            value: String::new(),
            label: "this model has no reasoning setting.".into(),
            desc: crate::models::long_name(&model),
            mark: None,
        });
        return out;
    }
    let default = crate::models::efforts(&model).1;
    for e in efforts.iter().filter(|e| matches(q, &[e])) {
        let mut desc = effort_hint(e).to_string();
        if *e == default {
            desc = if desc.is_empty() { "default".into() } else { format!("{} · default", desc) };
        }
        out.push(Choice {
            value: e.clone(),
            label: e.clone(),
            desc,
            mark: (*e == current).then(|| ("✓", theme::accent())),
        });
    }
    out
}

/// `/command <args>`: the popup of the argument being typed (BISE-117),
/// like the `/` one: tab completes (the next argument follows), ⏎ runs
/// the line when nothing required is left, else completes.
pub(crate) fn arg_items(app: &App) -> Vec<PopItem> {
    if !popup_open(app) {
        return Vec::new();
    }
    let Some((cmd, head, idx, word)) = arg_slot(&app.ed.text) else {
        return Vec::new();
    };
    if sb::release::DEV_COMMANDS.iter().any(|c| c.name == cmd.name) && !sb::release::dev(app) {
        return Vec::new();
    }
    let Some(&arg) = cmd.args.get(idx) else {
        return Vec::new();
    };
    let next = cmd.args.get(idx + 1);
    // a note, or words after the value (`/model <m> [default]`), are
    // optional: ⏎ runs the line
    let runs = matches!(next, None | Some(Arg::Note) | Some(Arg::Words(_)));
    choices(app, arg, word)
        .into_iter()
        .map(|c| {
            let line = format!("{head}{}", c.value);
            let (fill, run) = if c.value.is_empty() {
                (app.ed.text.clone(), None)
            } else if c.value.starts_with('/') {
                // a command of its own (`/model`'s `+ another provider…`)
                (c.value.clone(), Some(c.value.clone()))
            } else if next.is_none() {
                (line.clone(), Some(line))
            } else {
                (format!("{line} "), runs.then_some(line))
            };
            PopItem {
                label: c.label,
                desc: c.desc,
                mark: c.mark,
                fill_cursor: fill.chars().count(),
                fill,
                run,
                closable: true,
                path: None,
                folder: false,
            }
        })
        .collect()
}

/// The dim line above the argument popup of `/archive` and `/restore`
/// (designer): `archive which agent?`, `restore which agent?`. Not a
/// row: ⏎ still takes the first agent (the one in view).
pub(crate) fn popup_title(app: &App) -> Option<&'static str> {
    if !popup_open(app) {
        return None;
    }
    match arg_slot(&app.ed.text)? {
        (c, _, 0, _) if c.name == "/archive" => Some("archive which agent?"),
        (c, _, 0, _) if c.name == "/restore" => Some("restore which agent?"),
        (c, _, 0, _) if c.name == "/diff" => Some("diff which branch vs main?"),
        _ => None,
    }
}

/// A `@`, `$`, `:` or argument popup may complete the draft: not while
/// a history line is recalled, nor after Esc closed the list on this text.
pub(crate) fn popup_open(app: &App) -> bool {
    !app.ed.browsing() && app.popup_dismissed.as_deref() != Some(app.ed.text.as_str())
}

/// Artifacts listed after the agents in the `@` popup.
const ARTIFACT_ROWS: usize = 8;
/// Files listed after the agents in the `@` popup.
const FILE_ROWS: usize = 50;
/// The file and folder marks of the `@` popup.
const FILE_MARK: &str = "▪";
const DIR_MARK: &str = theme::G_CLOSED; // a folder opens: the disclosure mark

/// `@word` at the start or inline: the live agents (Switchboard), then
/// the files and folders of the workspace (files.rs). A file inserts its
/// relative path, an agent `@name`.
pub(crate) fn at_items(app: &App) -> Vec<PopItem> {
    let token = popup_open(app).then(|| files::token(&app.ed.text, app.ed.cursor)).flatten();
    let Some((start, q)) = token else {
        files::forget_dirs(); // the popup closed: its folder listings go
        return Vec::new();
    };
    let fill = |ins: &str| files::complete(&app.ed.text, start, app.ed.cursor, ins);
    let agents = sb::mentions(app, &q).into_iter().map(|m| {
        let (fill, fill_cursor) = fill(&m.completion());
        PopItem {
            label: format!("@{}", m.name),
            desc: if m.objective.is_empty() {
                m.status.clone()
            } else {
                format!("{} · {}", m.status, m.objective)
            },
            mark: Some(m.glyph(app.tick, app.motion)),
            fill,
            fill_cursor,
            run: None,
            closable: true,
            path: None,
            folder: false,
        }
    });
    // site/m/artifacts C: the artifacts by name, after the agents; a
    // pick puts a `↗ title` chip in your message, never `@name`
    let now = crate::when::now_ms();
    let made: Vec<PopItem> = crate::artifacts::all()
        .into_iter()
        .filter(|a| !q.contains('/') && matches(&q, &[&a.title, &a.agent, &a.kind, &a.id]))
        .take(ARTIFACT_ROWS)
        .map(|a| {
            let (fill, fill_cursor) = files::replace_token(&app.ed.text, start, app.ed.cursor, "");
            let who = if a.agent.is_empty() { a.by.clone() } else { a.agent.clone() };
            PopItem {
                label: a.title.clone(),
                desc: format!("{} · {} · {}", a.kind_word(), who, crate::artifacts::ago_words(a.ts_ms, now)),
                mark: Some(("↗", theme::accent())),
                fill,
                fill_cursor,
                run: None,
                closable: true,
                path: Some(crate::artifacts::url_of(&a.id, None)),
                folder: false,
            }
        })
        .collect();
    let agents = agents.chain(made);
    // the Switchboard workspace, else the folder the TUI runs in
    let root = sb::workspace(app)
        .map(std::path::PathBuf::from)
        .or_else(|| std::env::current_dir().ok())
        .unwrap_or_default();
    // `@../`, `@~/`, `@/`: the typed folder's own listing, no index; a
    // folder browsed: its own row last (files::pick, the window's too)
    let files::Pick { hits, this, outside, locked } = files::pick(&root, &q, FILE_ROWS);
    // an outside path is sent in a form the tools read (`~/` expanded)
    let sent = |p: &str| if outside { files::sent_path(p) } else { p.to_string() };
    let files = hits.into_iter().map(|h| {
        let (fill, fill_cursor) = if h.dir {
            files::replace_token(&app.ed.text, start, app.ed.cursor, &files::browse(&h.path))
        } else {
            fill(&files::reference(&sent(&h.path), false))
        };
        PopItem {
            label: format!("{}{}", h.path, if h.dir { "/" } else { "" }),
            // macOS asks for this folder once it is entered
            desc: if h.protected { "protected".into() } else { String::new() },
            mark: Some(if h.dir { (theme::glyph(DIR_MARK), theme::accent()) } else { (FILE_MARK, theme::dim()) }),
            fill,
            fill_cursor,
            run: None,
            closable: true,
            path: Some(if h.dir { h.path } else { sent(&h.path) }),
            folder: h.dir,
        }
    });
    let this = this.map(|path| {
        let sent = sent(&path);
        let (fill, fill_cursor) = if sent == "/" { fill("/") } else { fill(&files::reference(&sent, true)) };
        PopItem {
            label: if path == "/" { path.clone() } else { format!("{path}/") },
            desc: if locked { "this folder · no access".into() } else { "this folder".into() },
            mark: Some((theme::glyph(DIR_MARK), theme::accent())),
            fill,
            fill_cursor,
            run: None,
            closable: true,
            path: Some(sent),
            folder: false,
        }
    });
    agents.chain(files).chain(this).collect()
}

/// ← or Backspace while the `@` popup browses a folder (`@rust/tui/`):
/// the composer one folder up (`@rust/`), the popup still open. None
/// when the token does not end with `/` (the key edits as usual).
pub(crate) fn at_up(app: &App) -> Option<(String, usize)> {
    if !popup_open(app) || app.ed.anchor.is_some() {
        return None;
    }
    let (start, q) = files::token(&app.ed.text, app.ed.cursor)?;
    let up = files::parent_query(&q)?;
    Some(files::replace_token(&app.ed.text, start, app.ed.cursor, &files::browse(up)))
}

/// `$skill` anywhere in the draft: the skills of the index.
pub(crate) fn skill_items(app: &App) -> Vec<PopItem> {
    if !popup_open(app) {
        return Vec::new();
    }
    let Some((start, q)) = skills::token(&app.ed.text, app.ed.cursor) else {
        return Vec::new();
    };
    let ws = crate::sb::workspace(app)
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|| std::env::current_dir().unwrap_or_default());
    let all = skills::index(&ws);
    skills::filter(&all, &q)
        .into_iter()
        .map(|s| {
            let (fill, fill_cursor) = skills::complete(&app.ed.text, start, app.ed.cursor, &s.name);
            PopItem {
                label: format!("${}", s.name),
                desc: s.desc.clone(),
                mark: None,
                fill,
                fill_cursor,
                run: None,
                closable: true,
                path: None,
                folder: false,
            }
        })
        .collect()
}

/// `:name` anywhere in the draft: the matching emojis (emoji.rs).
pub(crate) fn emoji_items(app: &App) -> Vec<PopItem> {
    if !popup_open(app) {
        return Vec::new();
    }
    let Some((start, q)) = emoji::token(&app.ed.text, app.ed.cursor) else {
        return Vec::new();
    };
    emoji::filter(&q)
        .into_iter()
        .map(|(e, name)| {
            let (fill, fill_cursor) = emoji::complete(&app.ed.text, start, app.ed.cursor, e.glyph);
            PopItem {
                label: format!(":{}:", name),
                desc: e.desc.to_string(),
                mark: Some((e.glyph, theme::text())),
                fill,
                fill_cursor,
                run: None,
                closable: true,
                path: None,
                folder: false,
            }
        })
        .collect()
}

/// First visible row of a popup of `len` entries showing `rows`, so the
/// selection `sel` stays in view.
pub(crate) fn popup_top(sel: usize, len: usize, rows: usize) -> usize {
    if len <= rows {
        0
    } else {
        sel.min(len - 1).saturating_sub(rows - 1)
    }
}

/// One line typed by the user: `sb::handle_input` (the hub interprets it).
pub(crate) use crate::sb::handle_input;

#[cfg(test)]
mod popup_tests {
    use super::popup_top;

    #[test]
    fn the_selection_stays_in_view() {
        assert_eq!(popup_top(0, 5, 8), 0);
        assert_eq!(popup_top(4, 5, 8), 0);
        assert_eq!(popup_top(7, 12, 8), 0);
        assert_eq!(popup_top(8, 12, 8), 1);
        assert_eq!(popup_top(11, 12, 8), 4);
        assert_eq!(popup_top(99, 12, 8), 4); // clamped like the selection
    }
}

#[cfg(test)]
mod arg_tests {
    use super::*;
    use crate::sb::bench::{add_agent, set_model, set_status, set_versions_dev, test_app};

    /// The usage after `: /name ` in a command's description, as words.
    fn usage(c: &Cmd) -> Vec<&'static str> {
        c.desc
            .split_once(&format!(": {}", c.name))
            .map(|(_, u)| u.split_whitespace().collect())
            .unwrap_or_default()
    }

    /// The user's bug: after `/computer-use off` the menu still offered
    /// off/uninstall, and ⏎ on the bare command ran "off" again. Off: one
    /// row that runs the bare command (which turns it on); on: setup (bare),
    /// off, uninstall. The state is plugins.json's, as /computer-use writes it.
    #[test]
    fn computer_use_rows_follow_the_plugin_state() {
        let rows = |on: bool, q: &str| -> Vec<(String, String)> {
            computer_use_choices(on, q).into_iter().map(|c| (c.label, c.value)).collect()
        };
        let bare = "/computer-use".to_string();
        assert_eq!(rows(false, ""), vec![("on".to_string(), bare.clone())]);
        assert!(rows(false, "of").is_empty(), "never 'off' when it is off");
        let on = rows(true, "");
        assert_eq!(on[0], ("setup".to_string(), bare));
        assert_eq!(on.iter().map(|r| r.0.as_str()).collect::<Vec<_>>(), ["setup", "off", "uninstall"]);
        // the state file: off, then the bare command turns it back on
        let d = std::env::temp_dir().join(format!("cu-menu-{}", std::process::id()));
        let p = d.join("plugins.json");
        assert!(!crate::computer_use::on_in(&p), "off before /computer-use");
        bend_plugins::state::set_enabled(&p, "computer", true).unwrap();
        assert!(crate::computer_use::on_in(&p));
        bend_plugins::state::set_enabled(&p, "computer", false).unwrap();
        assert!(!crate::computer_use::on_in(&p), "off after /computer-use off");
        bend_plugins::state::set_enabled(&p, "computer", true).unwrap();
        assert!(crate::computer_use::on_in(&p), "on again after the bare /computer-use");
        let _ = std::fs::remove_dir_all(&d);
    }

    /// BISE-117: a command that takes a parameter completes it. Its usage
    /// (in `desc`, what /help shows) and its `args` agree: one completer
    /// per parameter (free text last, several words allowed), the first
    /// one never free text, and the literal words of a `[a|b|<x>]` choice
    /// all offered by it (a version's words come from the hub's list).
    #[test]
    fn every_parameter_has_a_completer() {
        for c in COMMANDS {
            let u = usage(c);
            assert_eq!(u.is_empty(), c.args.is_empty(), "{}: usage {:?} vs args {:?}", c.name, u, c.args);
            let Some(first) = c.args.first() else { continue };
            assert!(!matches!(first, Arg::Text | Arg::Note), "{}: its first parameter completes", c.name);
            let text_last = matches!(c.args.last(), Some(Arg::Text | Arg::Note));
            if text_last {
                assert!(u.len() >= c.args.len(), "{}: {:?}", c.name, u);
            } else {
                assert_eq!(u.len(), c.args.len(), "{}: {:?}", c.name, u);
            }
            for (a, w) in c.args.iter().zip(&u) {
                let offered: Vec<&str> = match a {
                    Arg::Words(ws) => ws.iter().map(|(w, _)| *w).collect(),
                    _ => Vec::new(),
                };
                let literal = w.trim_matches(|ch| ch == '[' || ch == ']');
                if matches!(a, Arg::Words(_)) {
                    for alt in literal.split('|').filter(|x| !x.starts_with('<') && *x != "name:") {
                        assert!(offered.contains(&alt), "{}: {} not offered", c.name, alt);
                    }
                }
            }
        }
    }

    /// From an agent's view, `/archive` lists it first (`· in view`,
    /// ⏎ archives it), and `/restore` the archived agent in view; the
    /// titles say what the list is for. From main, the usual order.
    #[test]
    fn the_agent_in_view_comes_first() {
        let mut app = test_app();
        for (n, o) in [("auth-fix", "fix the login"), ("docs", "the release note"), ("bench", "time it")] {
            add_agent(&mut app, n, o);
        }
        for n in ["old", "older"] {
            add_agent(&mut app, n, "gone");
            set_status(&mut app, n, "archived");
        }
        let labels = |v: &[PopItem]| v.iter().map(|i| i.label.clone()).collect::<Vec<_>>();
        assert_eq!(labels(&items(&mut app, "/archive ")), ["auth-fix", "docs", "bench"]);
        assert!(items(&mut app, "/archive ").iter().all(|i| !i.desc.contains("in view")));
        crate::sb::focus(&mut app, "docs");
        let a = items(&mut app, "/archive ");
        assert_eq!(labels(&a), ["docs", "auth-fix", "bench"]);
        assert_eq!((a[0].desc.as_str(), a[0].run.as_deref()), ("· in view · working · the release note", Some("/archive docs")));
        assert_eq!(a[1].desc, "· working · fix the login", "the same ' · ' after every name");
        assert_eq!(popup_title(&app), Some("archive which agent?"));
        // a query the agent in view does not match: the others, as usual
        assert_eq!(labels(&items(&mut app, "/archive time")), ["bench"]);
        // /restore: the archived one in view first
        let before = labels(&items(&mut app, "/restore "));
        assert_eq!(before.len(), 2);
        let last = before[1].clone();
        crate::sb::focus(&mut app, &last);
        let r = items(&mut app, "/restore ");
        assert_eq!(r[0].label, last);
        assert!(r[0].desc.starts_with("· in view · "), "{}", r[0].desc);
        assert_eq!(r[0].run.as_deref(), Some(format!("/restore {last}").as_str()));
        assert_eq!(popup_title(&app), Some("restore which agent?"));
        app.ed.text = "/rename ".into();
        app.ed.cursor = 8;
        assert_eq!(popup_title(&app), None, "only /archive and /restore have one");
    }

    fn items(app: &mut App, text: &str) -> Vec<PopItem> {
        app.ed.text = text.into();
        app.ed.cursor = text.chars().count();
        popup_items(app)
    }

    /// Each kind of argument offers its values after the command's name,
    /// filtered by the word typed; tab fills (a space when an argument
    /// follows), ⏎ runs when nothing required is left.
    #[test]
    fn the_arguments_complete() {
        let mut app = test_app();
        add_agent(&mut app, "auth-fix", "fix the login");
        add_agent(&mut app, "docs", "the release note");
        set_status(&mut app, "docs", "archived");
        let labels = |v: &[PopItem]| v.iter().map(|i| i.label.clone()).collect::<Vec<_>>();
        assert_eq!(labels(&items(&mut app, "/theme ")), ["auto", "light", "dark"]);
        let t = items(&mut app, "/theme li");
        assert_eq!((t[0].fill.as_str(), t[0].run.as_deref()), ("/theme light", Some("/theme light")));
        assert_eq!(labels(&items(&mut app, "/archive ")), ["auth-fix"], "live tasks only");
        assert_eq!(labels(&items(&mut app, "/isolate log")), ["auth-fix"], "the objective matches");
        assert_eq!(labels(&items(&mut app, "/restore ")), ["docs"], "archived only");
        let r = items(&mut app, "/rename a");
        assert_eq!((r[0].fill.as_str(), r[0].run.as_deref()), ("/rename auth-fix ", None), "a new name follows");
        assert!(items(&mut app, "/rename auth-fix x").is_empty(), "free text: nothing to complete");
        let n = items(&mut app, "/new ");
        assert_eq!((labels(&n), n[0].fill.as_str()), (vec!["-w".to_string()], "/new -w "));
        assert!(items(&mut app, "/new fix the tests").is_empty());
        assert_eq!(labels(&items(&mut app, "/plugins ")), ["list", "enable", "disable", "login", "logout"]);
        let v = items(&mut app, "/restart ");
        assert_eq!(v[0].label, "current");
        assert!(v.iter().any(|i| i.label == "…" && i.run.is_none()), "the versions load");
        // qa-explore H: outside bise's sources /restart refuses a commit,
        // so it offers none; /version still lists them
        set_versions_dev(&mut app, false);
        assert_eq!(labels(&items(&mut app, "/restart ")), ["current"]);
        assert_eq!(labels(&items(&mut app, "/version ")), ["abc1234"]);
        set_versions_dev(&mut app, true);
        assert_eq!(labels(&items(&mut app, "/restart ")), ["current", "abc1234"]);
        assert!(items(&mut app, "/agents ").is_empty(), "no argument, no popup");
        // voice-menu (designer): /voice is one row with no argument
        // popup: ⏎ opens the one voice screen
        assert!(items(&mut app, "/voice ").is_empty(), "no settings / setup rows");
        let v = items(&mut app, "/vo");
        assert_eq!(labels(&v), ["/voice"]);
        assert_eq!(v[0].desc, "dictation and voice mode: the model, the voice, the language");
        assert_eq!(v[0].run.as_deref(), Some("/voice"), "one ⏎ runs it");
        // BISE-135: /model and /reasoning, for the agent in view
        app.sb.focus = "auth-fix".into();
        set_model(&mut app, "auth-fix", "foundry/claude-opus-5-5", "high");
        let m = items(&mut app, "/model ");
        assert_eq!((m[0].label.as_str(), m[0].run.as_deref()), ("model for auth-fix", None), "a note: which agent");
        // BISE-301: grouped under their provider's name (a row that picks
        // nothing), the ids without it; the current one's ✓ in the indent
        let opus = m.iter().find(|i| i.label.trim_end() == "claude-opus-5-5" && i.fill.contains("foundry/")).unwrap();
        assert_eq!(opus.mark.map(|x| x.0), Some("✓"), "the current one");
        let head = m.iter().position(|i| i.label == "Anthropic").unwrap();
        assert!(m[head].run.is_none() && m[head].fill == "/model ", "a header picks nothing");
        assert!(m[head + 1].label.starts_with("  claude-"), "{:?}", labels(&m));
        assert!(m.iter().any(|i| i.label == "aliases"));
        assert_eq!(opus.fill, "/model foundry/claude-opus-5-5 ");
        assert_eq!(opus.run.as_deref(), Some("/model foundry/claude-opus-5-5"), "default is optional: ⏎ runs");
        assert!(m.iter().any(|i| i.label.trim_end() == "  opus-5.5"), "the aliases");
        let s = items(&mut app, "/model sonnet");
        let models: Vec<&PopItem> = s[1..].iter().filter(|i| i.run.is_some()).collect();
        assert!(!models.is_empty() && models.iter().all(|i| i.label.contains("sonnet")), "{:?}", labels(&s));
        assert!(s[1..].iter().all(|i| i.mark.is_none()));
        assert_eq!(labels(&items(&mut app, "/model anthropic/claude-sonnet-4-5 ")), ["default"]);
        // BISE-289: an id the list does not have, last, as is; no
        // provider typed: the current model's
        let f = items(&mut app, "/model claude-mythos-9");
        assert_eq!(labels(&f), ["model for auth-fix", "no listed model matches.", "+ use foundry/claude-mythos-9"]);
        assert_eq!((f[1].run.as_deref(), f[2].run.as_deref()), (None, Some("/model foundry/claude-mythos-9")));
        let f = items(&mut app, "/model openai/gpt-6");
        assert_eq!(f.last().unwrap().label, "+ use openai/gpt-6");
        assert!(f.iter().any(|i| i.label.trim_end() == "  gpt-6-astra"), "the listed ones that match stay");
        assert!(!labels(&f).iter().any(|l| l == "no listed model matches."));
        // a listed id: no extra row
        assert!(!labels(&items(&mut app, "/model openai/gpt-6-astra")).iter().any(|l| l.starts_with("+ use")));
        // BISE-294: the providers set up only; the others one row away
        let last = |m: &[PopItem]| (m.last().unwrap().label.clone(), m.last().unwrap().run.clone());
        // BISE-298: then every role's model
        let all = items(&mut app, "/model ");
        assert_eq!(last(&all), ("every role…".to_string(), Some("/models".to_string())));
        assert_eq!(last(&all[..all.len() - 1]), ("+ another provider…".to_string(), Some("/provider".to_string())));
        crate::models::TEST_READY.with(|r| *r.borrow_mut() = Some(vec!["foundry".into()]));
        let m = items(&mut app, "/model ");
        assert!(m.iter().any(|i| i.fill == "/model foundry/claude-opus-5-5 "));
        assert!(!m.iter().any(|i| i.fill.starts_with("/model openai/") || i.fill.starts_with("/model openrouter/")), "{:?}", labels(&m));
        let more = &m[m.len() - 2];
        assert_eq!((more.label.as_str(), more.run.as_deref(), more.fill.as_str()), ("+ another provider…", Some("/provider"), "/provider"));
        assert!(more.desc.starts_with("Anthropic, OpenAI, Google AI Studio"), "{}", more.desc);
        let f = items(&mut app, "/model openrouter/x-ai/grok-9");
        assert!(f.iter().any(|i| i.label == "+ set up OpenRouter for openrouter/x-ai/grok-9" && i.desc == "no key yet: i'll ask for one"), "{:?}", labels(&f));
        crate::models::TEST_READY.with(|r| *r.borrow_mut() = None);
        let r = items(&mut app, "/reasoning ");
        assert_eq!(labels(&r), ["reasoning for auth-fix", "none", "low", "medium", "high", "max"]);
        let high = r.iter().find(|i| i.label == "high").unwrap();
        assert_eq!((high.mark.map(|x| x.0), high.desc.as_str()), (Some("✓"), "deeper, slower · default"));
        assert_eq!(items(&mut app, "/reasoning lo")[1].run.as_deref(), Some("/reasoning low"));
        // main, on a model with two words; one without reasoning
        add_agent(&mut app, "main", "");
        app.sb.focus = "main".into();
        set_model(&mut app, "main", "mistral/zai-glm-5-3", "none");
        let r = items(&mut app, "/reasoning ");
        assert_eq!(labels(&r), ["reasoning for main", "none", "high"]);
        assert_eq!(r[1].mark.map(|x| x.0), Some("✓"));
        set_model(&mut app, "main", "mistral/mistral-large-latest", "");
        let r = items(&mut app, "/reasoning ");
        assert_eq!(labels(&r), ["reasoning for main", "this model has no reasoning setting."]);
        assert!(r.iter().all(|i| i.run.is_none()));
        assert!(items(&mut app, "/theme light\nx").is_empty());
    }
}
