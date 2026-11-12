//! `sb <args>` as its typed request (`bise_proto::sb::Req`, issue 27):
//! the arguments are read here, with the agent's own cwd, clock and
//! sandbox; the hub decodes the same type. A new command is one variant
//! in proto, one arm here, one handler in the hub.

use super::*;
use bise_proto::sb::{Artifact, ArtifactPick, Ask, Brief, Every, History, Inspect, PagePublish, Req};

const EVERY_USAGE: &str = "usage: sb every <10m|1h|day 07:30> \"<message>\" [--until <18:00|tomorrow 18:00|2h>] [--times <n>] [--to <agent>] [--page <id>] [--name <2-5 words>] | sb every | sb every --show <id> | sb every --stop <id>";

/// The request for `sb <args>`.
pub fn build(args: &[String]) -> Result<Req, String> {
    let Some(cmd) = args.first() else {
        return Err(usage());
    };
    let rest = &args[1..];
    let agent = |pos: &[String], usage: &str| agent_arg(pos, usage);
    Ok(match cmd.as_str() {
        "list" | "tasks" => {
            parse_args(rest, &[], &[])?;
            if cmd == "list" {
                Req::List
            } else {
                Req::Tasks
            }
        }
        "history" => Req::History(history(rest)?),
        "show" => {
            let (pos, o) = parse_args(rest, &["context"], &[])?;
            let context = if o.contains_key("context") {
                str_of(&o, "context").parse::<u64>().map_err(|_| "--context expects a number".to_string())?
            } else {
                crate::search::DEFAULT_CONTEXT as u64
            };
            // msg-forward: `sb show m_<n>`, the message where its recipient got it
            if let [one] = pos.as_slice() {
                if let Some(n) = crate::core::parse_msg_id(one) {
                    return Ok(Req::Show { agent: String::new(), pos: 0, context: Some(context), msg: Some(n) });
                }
            }
            let (agent, pos) = crate::search::parse_ref(&pos).ok_or("usage: sb show <agent>#<pos> | m_<n> [--context <n>]")?;
            Req::Show { agent, pos: pos as u64, context: Some(context), msg: None }
        }
        "send" => send(rest)?,
        // desktop S2 (bise's main): the projects, his words forwarded by
        // reference, bise's own question
        "project" => {
            let (pos, o) = parse_args(rest, &["input"], &[])?;
            let usage = "usage: sb project list | send <project> --input m_<n> | ask <project> \"<question>\"";
            match pos.first().map(String::as_str) {
                None | Some("list") => Req::ProjectList,
                Some("send") => {
                    let msg = crate::core::parse_msg_id(&str_of(&o, "input")).ok_or(usage)?;
                    Req::ProjectSend { project: pos.get(1).ok_or(usage)?.clone(), msg }
                }
                Some("ask") => Req::ProjectAsk { project: pos.get(1).ok_or(usage)?.clone(), text: text_of(pos.get(2..).unwrap_or(&[]))? },
                Some(_) => return Err(usage.into()),
            }
        }
        "ask" => {
            let (pos, o) = parse_args(rest, &["timeout"], &[])?;
            Req::Ask { to: agent(&pos, "usage: sb ask <agent> \"<question>\"")?, text: text_of(&pos[1..])?, timeout_s: timeout_of(&o) }
        }
        "wait" => {
            let (pos, o) = parse_args(rest, &["timeout"], &[])?;
            let msg = crate::core::parse_msg_id(pos.first().ok_or("usage : sb wait m_<n>")?).ok_or("usage : sb wait m_<n>")?;
            Req::Wait { msg, timeout_s: timeout_of(&o) }
        }
        "status" => {
            let (pos, o) = parse_args(rest, &["note"], &[])?;
            let usage = "usage: sb status working|done|blocked";
            let status = serde_json::from_value(json!(pos.first().ok_or(usage)?)).map_err(|_| usage.to_string())?;
            Req::Status { status, note: str_of(&o, "note") }
        }
        "flow" => match parse_args(rest, &[], &[])?.0.as_slice() {
            [] => Req::Flow { set: String::new() },
            [m] => {
                crate::devflow::parse_mode(m).map_err(|_| "usage: sb flow [pr|trunk]".to_string())?;
                Req::Flow { set: m.clone() }
            }
            _ => return Err("usage: sb flow [pr|trunk]".into()),
        },
        "worktree" => {
            // BISE-136: gate.sh new/done tell the hub where the agent works
            let (pos, _) = parse_args(rest, &[], &[])?;
            let p = pos.first().ok_or("usage: sb worktree <path>|none")?;
            worktree_exists(p)?;
            Req::Worktree { path: p.clone() }
        }
        "follow" => {
            let (pos, o) = parse_args(rest, &[], &["off"])?;
            Req::Follow { agent: agent(&pos, "usage: sb follow <agent> [--off]")?, on: !o.contains_key("off"), hub: String::new() }
        }
        "report" => {
            let (pos, o) = parse_args(rest, &["decision", "step", "result"], &[])?;
            // S10: --step n/m; emitter 5: --result, proto's DiffResult
            let step = if o.contains_key("step") { Some(step_of(&str_of(&o, "step"))?) } else { None };
            let result = if o.contains_key("result") { Some(result_of(&str_of(&o, "result"))?) } else { None };
            let kind = pos.first().ok_or("usage: sb report <kind> \"<summary>\"")?.clone();
            if !["progress", "done", "failed", "blocked"].contains(&kind.as_str()) {
                return Err(format!("unknown report kind: {kind} (progress|done|failed|blocked)"));
            }
            Req::Report { kind, summary: text_of(&pos[1..])?, decisions: list_of(o.get("decision")), step, result }
        }
        "spawn" => spawn(rest)?,
        "move" => {
            let (pos, _) = parse_args(rest, &[], &[])?;
            let usage = "usage: sb move <agent> new|shared|<agent>|<branch>";
            Req::Move { agent: agent(&pos, usage)?, place: pos.get(1).ok_or(usage)?.clone() }
        }
        "feature" => {
            // dev-flow §5.1: sb feature [new|sync|ready|merge|drop|retire|list] <name>
            let (pos, _) = parse_args(rest, &[], &[])?;
            match pos.as_slice() {
                [] => Req::Feature { op: "list".into(), name: String::new() },
                [op] if op == "list" => Req::Feature { op: "list".into(), name: String::new() },
                [op, name] if ["new", "sync", "ready", "merge", "drop", "retire"].contains(&op.as_str()) => Req::Feature { op: op.clone(), name: name.clone() },
                _ => return Err("usage: sb feature [new|sync|ready|merge|drop|retire|list] <name>".into()),
            }
        }
        "every" => Req::Every(every(rest)?),
        "wake" => Req::Wake(wake_req(rest)?),
        "taste" => Req::Taste(keeps_req(cmd, rest)?),
        "people" => Req::People(keeps_req(cmd, rest)?),
        "land" => {
            let (pos, o) = parse_args(rest, &["add"], &["here"])?;
            // --add paths, absolute: the hub resolves them in the place
            let cwd = std::env::current_dir().unwrap_or_default();
            let add = list_of(o.get("add")).iter().map(|p| cwd.join(p).to_string_lossy().to_string()).collect();
            Req::Land { here: o.contains_key("here"), message: pos.join(" "), add }
        }
        "interrupt" | "drop" | "restore" | "isolate" => {
            let (pos, _) = parse_args(rest, &[], &[])?;
            let agent = agent(&pos, &format!("usage: sb {} <agent>", cmd))?;
            match cmd.as_str() {
                "interrupt" => Req::Interrupt { agent },
                "drop" => Req::Drop { agent },
                "restore" => Req::Restore { agent },
                _ => Req::Isolate { agent },
            }
        }
        "stop" => {
            let (pos, _) = parse_args(rest, &[], &[])?;
            Req::Stop { agent: agent(&pos, "usage: sb stop <agent> \"<reason>\"")?, reason: pos[1..].join(" ") }
        }
        "close" => {
            let (pos, _) = parse_args(rest, &[], &[])?;
            let card = pos.first().and_then(|c| c.trim_start_matches('#').parse::<u64>().ok()).ok_or("usage: sb close <notification> [\"<note>\"]")?;
            Req::Close { card, note: pos[1..].join(" ") }
        }
        "rename" => match parse_args(rest, &[], &[])?.0.as_slice() {
            [a, b] => Req::Rename { agent: a.trim_start_matches('@').into(), new_name: b.trim_start_matches('@').into() },
            _ => return Err("usage: sb rename <agent> <new-name>".into()),
        },
        // BISE-299 `sb notify --withdraw N "why"`
        "notify" if rest.iter().any(|a| a == "--withdraw" || a.starts_with("--withdraw=")) => {
            let (pos, o) = parse_args(rest, &["withdraw"], &[])?;
            let card = o
                .get("withdraw")
                .and_then(|c| c.as_str())
                .and_then(|c| c.trim_start_matches('#').parse::<u64>().ok())
                .ok_or("usage: sb notify --withdraw <notification> \"<why>\"")?;
            Req::Withdraw { card, why: pos.join(" ") }
        }
        "notify" => notify_req(rest)?,
        "inspect" => Req::Inspect(inspect(rest)?),
        // agent-made pages (docs/ambient-pages.md §2.2)
        "page" => page_req(rest)?,
        "artifact" => Req::Artifact(artifact(rest)?),
        "help" | "--help" | "-h" => return Err(usage()),
        other => return Err(format!("unknown command: {}\n{}", other, usage())),
    })
}

/// `sb send <agent> "<text>"`, or a model switch alone (issue #4).
fn send(rest: &[String]) -> Result<Req, String> {
    let (pos, o) = parse_args(rest, &["reply-to", "mode", "why", "model", "effort", "attach", "forward"], &["expect-reply"])?;
    let to = agent_arg(&pos, "usage: sb send <agent> \"<text>\"")?;
    let fwd = forward_of(&o)?;
    // desktop S2: `@bise` (bise's question in a project hub) is a sender
    // of its own, not the agent `bise`
    let to = if pos.first().map(String::as_str) == Some("@bise") { "@bise".to_string() } else { to };
    // issue #4: `--model`/`--effort` move the task from its next turn;
    // the text is optional then (a switch alone)
    let (model, effort) = (str_of(&o, "model"), str_of(&o, "effort"));
    let switch = (!model.is_empty() || !effort.is_empty()).then(|| (model.clone(), effort.clone()));
    if switch.is_some() && pos.len() < 2 && fwd.is_none() {
        return Ok(Req::Switch { to, model, effort });
    }
    // msg-forward: with `--forward`, the text is the forwarder's note, optional
    let text = if fwd.is_some() { pos[1..].join(" ") } else { text_of(&pos[1..])? };
    let attach = attach::of(&o)?;
    let queued = match str_of(&o, "mode").as_str() {
        "" | "steer" => false,
        "queued" => true,
        m => return Err(format!("unknown mode: {} (steer|queued)", m)),
    };
    let reply_to = if o.contains_key("reply-to") {
        Some(crate::core::parse_msg_id(&str_of(&o, "reply-to")).ok_or("--reply-to expects a message id (m_<n>)")?)
    } else {
        None
    };
    // main answering a task for the user: the reason, shown to the user
    // in main's feed (C2 `answered`)
    Ok(Req::Send { to, text, expect_reply: o.contains_key("expect-reply"), reply_to, queued, why: str_of(&o, "why"), switch, attach, parts: String::new(), fwd })
}

/// msg-forward: `--forward m_<n>` (one), never with `--attach` (v1: two
/// numberings of `[Image #k]` in one message).
fn forward_of(o: &Map<String, Value>) -> Result<Option<u64>, String> {
    if !o.contains_key("forward") {
        return Ok(None);
    }
    if list_of(o.get("forward")).len() > 1 {
        return Err("--forward: one message per forward".into());
    }
    if o.contains_key("attach") {
        return Err("--forward and --attach go in two messages".into());
    }
    let id = str_of(o, "forward");
    crate::core::parse_msg_id(&id).map(Some).ok_or_else(|| format!("--forward expects a message id (m_<n>), not {id:?}"))
}

/// `sb spawn <name> --objective …` (RFC 0001 §7.1, dev-flow §3.1, §5.1).
fn spawn(rest: &[String]) -> Result<Req, String> {
    let (pos, o) = parse_args(
        rest,
        &["objective", "context", "constraint", "done-when", "report-format", "place", "feature", "model", "effort", "profile", "attach", "forward", "folder"],
        &["worktree", "with-changes"],
    )?;
    let attach = attach::of(&o)?;
    let fwd = forward_of(&o)?;
    // dev-flow §5.1: its own worktree from the feature's tip
    let feature = str_of(&o, "feature");
    if !feature.is_empty() && (o.contains_key("place") || o.contains_key("worktree")) {
        return Err("sb spawn: --feature gives the task its own worktree; no --place or --worktree with it".into());
    }
    let mut objective = str_of(&o, "objective");
    if objective.is_empty() && pos.len() > 1 {
        objective = pos[1..].join(" ");
    }
    // msg-forward: the forwarded message is the objective when none is given
    if objective.is_empty() && fwd.is_some() {
        objective = FORWARD_OBJECTIVE.to_string();
    }
    if objective.is_empty() {
        return Err("sb spawn: --objective (or --forward m_<n>) is required".into());
    }
    let some = |s: String| Some(s).filter(|s| !s.is_empty());
    // artifact-folders: the library folder its artifacts go to, by name
    let folder = match o.contains_key("folder") {
        true => Some(crate::artifacts_filing::folder_path(&str_of(&o, "folder")).map_err(|e| format!("sb spawn: {e}"))?),
        false => None,
    };
    let brief = Brief { objective, context: str_of(&o, "context"), constraints: list_of(o.get("constraint")), done_when: some(str_of(&o, "done-when")), report_format: some(str_of(&o, "report-format")), folder };
    // dev-flow §3.1: --place new (= --worktree) | <agent> | <branch>
    let place = str_of(&o, "place");
    // issue #4: the task's model, all optional (none: the agents default)
    let ask = Ask { model: str_of(&o, "model"), effort: str_of(&o, "effort"), profile: str_of(&o, "profile") };
    Ok(Req::Spawn {
        name: pos.first().cloned().unwrap_or_default(),
        brief,
        worktree: o.contains_key("worktree") || place == "new",
        with_changes: o.contains_key("with-changes"),
        place: if place == "new" { String::new() } else { place },
        feature,
        ask,
        attach,
        parts: String::new(),
        fwd,
    })
}

/// The objective of `sb spawn --forward` without `--objective`.
pub const FORWARD_OBJECTIVE: &str = "do what the forwarded message at the end of this brief asks";

/// `sb history "<words>"` (S2 C: `--project`, `--all` on bise's hub).
fn history(rest: &[String]) -> Result<History, String> {
    let (pos, o) = parse_args(rest, &["agent", "role", "since", "until", "limit", "page", "project"], &["archived", "live", "all"])?;
    if o.contains_key("project") && o.contains_key("all") {
        return Err("--project and --all exclude each other".into());
    }
    let list = |k: &str| -> Vec<String> {
        list_of(o.get(k)).iter().flat_map(|s| s.split(',').map(|x| x.trim().trim_start_matches('@').to_string())).filter(|x| !x.is_empty()).collect()
    };
    let roles = list("role");
    if let Some(r) = roles.iter().find(|r| crate::search::Role::parse(r).is_none()) {
        return Err(format!("unknown role: {} (user|assistant|message|tool|hub)", r));
    }
    let now = crate::util::now_ms();
    let time = |k: &str| o.contains_key(k).then(|| crate::search::parse_time(&str_of(&o, k), now)).transpose();
    if o.contains_key("archived") && o.contains_key("live") {
        return Err("--archived and --live exclude each other".into());
    }
    let archived = if o.contains_key("archived") { "only" } else if o.contains_key("live") { "no" } else { "" };
    let n = |k: &str, d: usize| -> Result<u64, String> {
        Ok(if o.contains_key(k) { str_of(&o, k).parse::<usize>().map_err(|_| format!("--{} expects a number", k))? } else { d } as u64)
    };
    Ok(History {
        query: text_of(&pos)?,
        agents: list("agent"),
        roles,
        since: time("since")?,
        until: time("until")?,
        archived: archived.into(),
        limit: Some(n("limit", crate::search::DEFAULT_HITS)?),
        page: Some(n("page", 1)?),
        project: o.contains_key("project").then(|| str_of(&o, "project")),
        all: o.contains_key("all"),
    })
}

/// `sb inspect <agent>`: a page of its thread, or the caller's origin.
fn inspect(rest: &[String]) -> Result<Inspect, String> {
    let (pos, o) = parse_args(rest, &["last", "limit", "query", "before", "after", "around", "at"], &["origin"])?;
    let n = if o.contains_key("limit") { str_of(&o, "limit") } else { str_of(&o, "last") };
    let anchor = |k: &str| -> Result<Option<String>, String> {
        if !o.contains_key(k) {
            return Ok(None);
        }
        let p = str_of(&o, k);
        crate::transcript::parse_pos(&p).ok_or(format!("--{} expects a position (#<n>)", k))?;
        Ok(Some(p))
    };
    Ok(Inspect {
        agent: agent_arg(&pos, "usage: sb inspect <agent>")?,
        last: Some(n.parse::<u64>().unwrap_or(20)),
        query: str_of(&o, "query"),
        before: anchor("before")?,
        after: anchor("after")?,
        around: anchor("around")?,
        at: anchor("at")?,
        origin: o.contains_key("origin"),
    })
}

/// `sb artifact add|keep|unkeep|move|folders|list|archive|unarchive`.
fn artifact(rest: &[String]) -> Result<Artifact, String> {
    let usage = "usage: sb artifact add <path or link> [--title \"<t>\"] [--kind <k>] [--folder <name|theme/task>] [--for-you] | sb artifact keep|unkeep <id>... | sb artifact move <id>... <folder> | --unfiled | sb artifact folders | sb artifact list [<words>] [--agent <a>] [--archived] | sb artifact archive|unarchive <id>... | --agent <a> | --before <YYYY-MM-DD[THH:MM]> | --kind <k>";
    let (pos, o) = parse_args(rest, &["title", "kind", "agent", "before", "folder"], &["archived", "unfiled", "for-you"])?;
    let opt = |k: &str| o.contains_key(k).then(|| str_of(&o, k));
    let agent = || opt("agent").map(|a| a.trim_start_matches('@').to_string());
    Ok(match pos.first().map(String::as_str) {
        Some("add") => {
            let [target] = &pos[1..] else { return Err(usage.into()) };
            let cwd = std::env::current_dir().map(|d| d.to_string_lossy().to_string()).unwrap_or_default();
            Artifact::Add { target: target.clone(), cwd, title: opt("title"), kind: opt("kind"), known_only: false, changed: None, folder: opt("folder"), for_you: o.contains_key("for-you") }
        }
        Some(d @ ("keep" | "unkeep")) if pos.len() > 1 => {
            let ids = pos[1..].iter().map(|i| i.trim_start_matches("artifact:").to_string()).collect();
            if d == "keep" {
                Artifact::Keep { ids }
            } else {
                Artifact::Unkeep { ids }
            }
        }
        Some("move") => {
            let ids = |p: &[String]| p.iter().map(|i| i.trim_start_matches("artifact:").to_string()).collect::<Vec<_>>();
            match (o.contains_key("unfiled"), &pos[1..]) {
                (true, [_, ..]) => Artifact::Move { ids: ids(&pos[1..]), folder: None },
                (false, [first @ .., folder]) if !first.is_empty() => Artifact::Move { ids: ids(first), folder: Some(folder.clone()) },
                _ => return Err(usage.into()),
            }
        }
        Some("folders") if pos.len() == 1 => Artifact::Folders,
        Some("list") => Artifact::List { words: pos[1..].join(" "), agent: agent(), archived: o.contains_key("archived") },
        Some(d @ ("archive" | "unarchive")) => {
            if pos.len() == 1 && !["agent", "kind", "before"].iter().any(|k| o.contains_key(*k)) {
                return Err(usage.into());
            }
            let before_ms = match opt("before") {
                Some(b) => Some(crate::artifacts::parse_before(&b).ok_or(format!("--before expects a date: YYYY-MM-DD or YYYY-MM-DDTHH:MM (UTC), not {}", b))?),
                None => None,
            };
            let ids = pos[1..].iter().map(|i| i.trim_start_matches("artifact:").to_string()).collect();
            let pick = ArtifactPick { ids, agent: agent(), kind: opt("kind"), before_ms };
            if d == "archive" {
                Artifact::Archive(pick)
            } else {
                Artifact::Unarchive(pick)
            }
        }
        _ => return Err(usage.into()),
    })
}

/// `sb every` (docs/ambient-roadmap.md B): the durations and times are
/// read here, with the agent's own clock and time zone (every.rs).
fn every(rest: &[String]) -> Result<Every, String> {
    let (pos, o) = parse_args(rest, &["until", "times", "to", "stop", "page", "name", "show"], &[])?;
    let id = |k: &str| str_of(&o, k).trim_start_matches('#').parse::<u64>().map_err(|_| EVERY_USAGE.to_string());
    if o.contains_key("stop") {
        return Ok(Every::Stop { id: id("stop")? });
    }
    if o.contains_key("show") {
        return Ok(Every::Show { id: id("show")? });
    }
    if pos.is_empty() || pos == ["list"] {
        return Ok(Every::List);
    }
    let now = crate::util::now_ms();
    let (every_ms, daily_min, words) = match pos[0].as_str() {
        "day" | "daily" => (None, Some(crate::every::parse_hhmm(pos.get(1).ok_or(EVERY_USAGE)?)? as u64), &pos[2..]),
        // the hub checks the minimum (1m)
        d => (Some(crate::every::parse_dur(d)?), None, &pos[1..]),
    };
    let until = str_of(&o, "until");
    let times = str_of(&o, "times");
    let times = if times.is_empty() { None } else { Some(times.parse::<u64>().ok().filter(|n| *n > 0).ok_or("sb every: --times is a number, at least 1")?) };
    let some = |s: String| Some(s.trim().to_string()).filter(|s| !s.is_empty());
    Ok(Every::Add {
        to: str_of(&o, "to"),
        text: text_of(words).map_err(|_| EVERY_USAGE.to_string())?,
        every_ms,
        daily_min,
        until_ms: if until.is_empty() { None } else { Some(crate::every::parse_until(&until, now)?) },
        times,
        page: some(str_of(&o, "page")),
        name: some(str_of(&o, "name")),
    })
}

/// A page publish's request: the file read here (the agent's path).
pub(super) fn publish(o: &Map<String, Value>, file: &str) -> Result<PagePublish, String> {
    let html = std::fs::read_to_string(file).map_err(|e| format!("{file}: {e}"))?;
    let done = list_of(o.get("notes-done")).iter().flat_map(|d| d.split(',').map(|x| x.trim().to_string()).collect::<Vec<_>>()).filter(|x| !x.is_empty()).collect();
    let mut answers = std::collections::BTreeMap::new();
    for a in list_of(o.get("note-answer")) {
        let (id, text) = a.split_once('=').ok_or(format!("--note-answer expects <note>=\"<answer>\", got {a:?}"))?;
        answers.insert(id.trim().to_string(), text.trim().trim_matches('"').to_string());
    }
    // where its text went (docs/ambient-roadmap.md A)
    let mut went = Vec::new();
    for w in list_of(o.get("went")) {
        went.push(serde_json::to_value(crate::pages::store::Went::parse(&w)?).map_err(|e| e.to_string())?);
    }
    // --public: in the static export the user's mirror publishes (his
    // setting); --private: out again
    let public = match (o.contains_key("public"), o.contains_key("private")) {
        (true, true) => return Err("--public or --private, not both".into()),
        (p, q) => (p || q).then_some(p),
    };
    let opt = |k: &str| o.contains_key(k).then(|| str_of(o, k));
    // --taste: it followed ~/bise/taste.md (the hub counts its rules)
    Ok(PagePublish { html, id: opt("id"), title: opt("title"), done, answers, went, taste: o.contains_key("taste"), public })
}
