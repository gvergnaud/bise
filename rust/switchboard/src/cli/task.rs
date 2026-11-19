//! `sb task <verb>`: tasks (docs/tasks.md), read into proto's typed
//! request. Out of cli.rs (its line limit).

use super::*;
use bise_proto::sb::Task;

pub(super) const TASK_USAGE: &str = "usage: sb task new \"<name>\" [--why \"<why>\"] [--done-when \"<item>\"]... | sb task list [--all] | sb task show <id> [--activity] | sb task done|reopen|resume|restart|ongoing <id> | sb task pause <id> [--until <18:00|tomorrow 09:00|2h>] | sb task rename <id> \"<name>\" | sb task why <id> \"<why>\" | sb task goal <id> \"<item>\" | sb task tick|untick <id> <n> | sb task merge <id> --into <id> | sb task unlink|link <id> <agent>";

/// `sb task move`'s usage (main only, on his word).
const MOVE_USAGE: &str = "usage: sb task move <task-id>... (--art <id> | --page <id> | --card <n>) | sb task move --none (--art <id> | --page <id> | --card <n>)";

/// `sb task <verb> ...` as its request.
pub(super) fn task_req(rest: &[String]) -> Result<Task, String> {
    let (pos, o) = parse_args(rest, &["why", "done-when", "until", "into", "art", "page", "card"], &["all", "activity", "log", "none"])?;
    let verb = pos.first().ok_or(TASK_USAGE)?.as_str();
    let id = || pos.get(1).map(|s| s.trim().to_string()).filter(|s| !s.is_empty()).ok_or_else(|| TASK_USAGE.to_string());
    let words = |from: usize| text_of(pos.get(from..).unwrap_or(&[]));
    Ok(match verb {
        "new" => Task::New { name: words(1)?, why: str_of(&o, "why"), goals: list_of(o.get("done-when")) },
        "list" => Task::List { all: o.contains_key("all") },
        // `--log`: the Activity's name before his m_66595, kept as its alias
        "show" => Task::Show { id: id()?, log: o.contains_key("activity") || o.contains_key("log") },
        "done" => Task::Done { id: id()? },
        "reopen" => Task::Reopen { id: id()? },
        "resume" => Task::Resume { id: id()? },
        "restart" => Task::Restart { id: id()? },
        "ongoing" => Task::Ongoing { id: id()? },
        "pause" => {
            let until = str_of(&o, "until");
            let until_ms = if until.is_empty() { None } else { Some(crate::every::parse_until(&until, crate::util::now_ms())?) };
            Task::Pause { id: id()?, until_ms }
        }
        "rename" => Task::Rename { id: id()?, name: words(2)? },
        "why" => Task::Why { id: id()?, text: words(2)? },
        "goal" => Task::Goal { id: id()?, text: words(2)? },
        "tick" | "untick" => {
            let n = pos.get(2).and_then(|n| n.trim_start_matches('#').parse::<u64>().ok()).filter(|n| *n > 0).ok_or("sb task tick <id> <n>: n is the done-when item, 1 the first")?;
            if verb == "tick" {
                Task::Tick { id: id()?, n }
            } else {
                Task::Untick { id: id()?, n }
            }
        }
        "merge" => {
            let into = str_of(&o, "into");
            if into.is_empty() {
                return Err(TASK_USAGE.into());
            }
            Task::Merge { id: id()?, into }
        }
        // main on his word: the agent is off the task (unlink) or back (link)
        "unlink" | "link" => {
            let agent = pos.get(2).map(|s| s.trim().trim_start_matches('@').to_string()).filter(|s| !s.is_empty()).ok_or(TASK_USAGE)?;
            if verb == "unlink" {
                Task::Unlink { id: id()?, agent }
            } else {
                Task::Link { id: id()?, agent }
            }
        }
        // main on his word: exactly these tasks (none with --none)
        "move" => {
            let works: Vec<String> = pos.get(1..).unwrap_or(&[]).iter().map(|s| s.trim().to_string()).filter(|s| !s.is_empty()).collect();
            let (art, page, card) = (str_of(&o, "art"), str_of(&o, "page"), str_of(&o, "card"));
            let card = if card.is_empty() { None } else { Some(card.trim_start_matches('#').parse::<u64>().map_err(|_| MOVE_USAGE.to_string())?) };
            let one = [!art.is_empty(), !page.is_empty(), card.is_some()].iter().filter(|x| **x).count() == 1;
            if !one || works.is_empty() == !o.contains_key("none") {
                return Err(MOVE_USAGE.into());
            }
            let page_ = !page.is_empty();
            Task::Move { card, art: if page_ { page } else { art }, page: page_, works }
        }
        _ => return Err(TASK_USAGE.into()),
    })
}
