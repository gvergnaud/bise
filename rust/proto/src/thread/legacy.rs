//! Every older text shape of the hub's transcript lines, read in one place
//! (issue 25, D2): `sb <kind> : <text>`, one colon and a space, each kind
//! with its own fields joined by ` : ` (a field's own ` : ` escaped as
//! ` \: `) and its newlines as `\n`. New lines are [`Feed`] JSON
//! (`sb <kind> :: <json>`, `core/feed.rs`); these are the lines written
//! before, which no transcript or journal ever rewrites (a position is an
//! id, a rollback reads them). [`upgrade`] reads one into the record it
//! means.
//!
//! Only shrinks: new code never writes a shape read here. Each shape has a
//! sample in `fixtures/legacy_lines.jsonl` (from the git history of every
//! writer and the tests' lines), with the record it upgrades to.
//!
//! The runtime's text observations (`  obs: <kind>: <text>`, [`obs`]) are
//! read here too, since issue 38 (their records: `thread/obs.rs`; samples
//! in `fixtures/obs_lines.jsonl`).
//!
//! Not a compat shim with an end date (issue 31's exempt list): old
//! transcripts stay on disk for good, so this reader lives as long as
//! history does. Never "expire" it.

use crate::core::feed::{self, Feed};
use crate::thread::lines::{field, is_msg_id, msg_id, unescape, Hub, Obs, TurnEnd};

/// An older line (`sb <kind> : <text>`) as the record it means; None when
/// it is not one (a new line, not a hub line), or its kind is unknown here,
/// or its text doesn't read as its kind (the reader shows its text).
pub fn upgrade(line: &str) -> Option<Feed> {
    let (kind, raw) = split(line.strip_prefix("sb ")?);
    of(kind, raw)
}

/// An older hub line after its `sb `, for [`crate::thread::lines::hub`]:
/// its record's view, else its kind and unescaped text (`Hub::Other`).
pub(crate) fn hub(rest: &str) -> Hub {
    let (kind, raw) = split(rest);
    match of(kind, raw) {
        Some(f) => crate::thread::lines::view(f),
        None => Hub::Other { kind: kind.to_string(), text: unescape(raw) },
    }
}

/// `<kind> : <text>`, or a kind alone.
fn split(rest: &str) -> (&str, &str) {
    rest.split_once(" : ").unwrap_or((rest, ""))
}

/// Up to `n` fields of a multi-field line (the last keeps its ` : `),
/// each unescaped, missing ones empty.
fn fields<const N: usize>(raw: &str) -> [String; N] {
    let mut f = raw.splitn(N, " : ").map(field);
    std::array::from_fn(|_| f.next().unwrap_or_default())
}

fn of(kind: &str, raw: &str) -> Option<Feed> {
    let text = unescape(raw);
    // a JSON payload that doesn't parse reads as null: its reader drops
    // it, as it always did (never shown as text)
    let json = |s: &str| serde_json::from_str::<serde_json::Value>(s).unwrap_or_default();
    let ids = || raw.split_whitespace().map(msg_id).collect::<Option<Vec<u64>>>().filter(|ids| !ids.is_empty());
    Some(match kind {
        "you" => Feed::You(feed::You { text, via: None }),
        "you-id" => Feed::YouId(feed::YouId { id: msg_id(raw.trim())? }),
        // the JSON as written: its own `\n` escapes are the JSON's
        "you-parts" => Feed::YouParts(feed::YouParts { parts: json(raw) }),
        "route-parts" => Feed::RouteParts(feed::RouteParts { parts: json(raw) }),
        "steered" => Feed::Steered(feed::Steered { ids: ids()? }),
        "steer-rx" => Feed::SteerRx(feed::SteerRx { ids: ids()? }),
        "context" => Feed::Context(feed::Context { context: json(raw) }),
        "undelivered" => {
            let [to, text] = fields(raw);
            Feed::Undelivered(feed::Undelivered { to, text })
        }
        // `{from} m_<n> : {text}`; an old direct reply `@{from} : {text}`
        // (its `@` kept: the reader knows it), no id
        "msg-in" => {
            let (head, body) = text.split_once(" : ").unwrap_or(("", &text));
            let (from, id) = head.split_once(' ').unwrap_or((head, ""));
            Feed::MsgIn(feed::MsgIn { from: from.into(), id: msg_id(id), body: body.into(), view: None })
        }
        // `{from} → {to} m_<n> : {text}`, no id before BISE-110
        "msg" => {
            let (head, body) = text.split_once(" : ").unwrap_or(("", &text));
            let (from, to) = head.split_once(" → ").unwrap_or((head, ""));
            let (to, id) = match to.rsplit_once(' ') {
                Some((t, id)) if is_msg_id(id) => (t, msg_id(id)),
                _ => (to, None),
            };
            Feed::Msg(feed::Msg { from: from.into(), to: to.into(), id, body: body.into() })
        }
        // `{to} : m_<n> : {1|0} : {text}`
        "sent" => {
            let f: Vec<String> = raw.splitn(4, " : ").map(field).collect();
            let [to, id, ask, body] = <[String; 4]>::try_from(f).ok()?;
            Feed::Sent(feed::Sent { to, id: msg_id(&id)?, ask: ask == "1", body })
        }
        "msg-you" => {
            let (from, body) = text.split_once(" : ").unwrap_or(("", &text));
            Feed::MsgYou(feed::MsgYou { from: from.into(), body: body.into() })
        }
        "answered" => {
            let [agent, question, answer, why] = fields(raw);
            Feed::Answered(feed::Answered { agent, question, answer, why })
        }
        // `#{id} {kind} @{agent} : {body}`
        "card" => {
            let (head, body) = text.split_once(" : ").unwrap_or((&text, ""));
            let mut w = head.split_whitespace();
            let id = w.next()?.strip_prefix('#')?.parse().ok()?;
            let kind = w.next().unwrap_or("question").to_string();
            let agent = w.next().map(|a| a.trim_start_matches('@')).unwrap_or_default().to_string();
            Feed::Card(feed::Card { v: 1, id, kind, agent, body: body.into() })
        }
        // `check|card|done <n> [<card>]`: the step (the call's number and
        // card are not read)
        "gate" => Feed::Gate(feed::Gate { step: text.split_whitespace().next().unwrap_or_default().into() }),
        // `{how} : {who} : {what} [: {note}]`
        "approval" => {
            let f: Vec<String> = raw.split(" : ").map(field).collect();
            let get = |i: usize| f.get(i).cloned().unwrap_or_default();
            Feed::Approval(feed::Approval { v: 1, how: get(0), who: get(1), what: get(2), note: get(3) })
        }
        // `#{id} {res}`
        "card-closed" => {
            let (id, res) = text.strip_prefix('#')?.split_once(' ')?;
            Feed::CardClosed(feed::CardClosed { id: id.parse().ok()?, res: res.trim().into() })
        }
        "route" => route(&text)?,
        // `{tone} : {number} : {url} : {text}`, the tone a color word
        "pr" => {
            let [tone, number, url, text] = fields(raw);
            let state = match tone.as_str() {
                "plain" => "news",
                "dim" => "done",
                "red" => "failing",
                _ => "unknown",
            };
            match number.parse() {
                Ok(number) => Feed::Pr(feed::Pr { v: 1, state: state.into(), number, url, text }),
                // no number: its text field alone, never lost
                Err(_) if !text.is_empty() => Feed::Info(feed::Info { text }),
                Err(_) => return None,
            }
        }
        // `{id} : {agent} : {title} : {kind} : {v}` (`v3` read too)
        "artifact" => {
            let [id, agent, title, kind, v] = fields(raw);
            if id.is_empty() {
                return None;
            }
            Feed::Artifact(feed::Artifact { id, agent, title, kind, version: v.trim_start_matches('v').parse().unwrap_or(1) })
        }
        // `{agent} : {target} : {from} : {sha} : {files} : {add} : {del}`
        "landed" => {
            let [agent, target, from, sha, files, add, del] = fields(raw);
            if sha.is_empty() {
                return None;
            }
            let num = |s: &str| s.trim().parse().unwrap_or(0);
            Feed::Landed(feed::Landed { v: 1, agent, target, from, sha, files: num(&files), add: num(&add), del: num(&del) })
        }
        "spawn" => Feed::Spawn(feed::Spawn { text }),
        "computer" => Feed::Computer(feed::Computer { text }),
        "direct" => Feed::Direct(feed::Direct { text }),
        "info" => Feed::Info(feed::Info { text }),
        "warn" => Feed::Warn(feed::Warn { text }),
        "scheduled" => Feed::Scheduled(feed::Scheduled { v: 1, timer: json(&text) }),
        "stopped" => Feed::Stopped(feed::Stopped { text }),
        "paused" => Feed::Paused(feed::Paused { text }),
        "wake" => Feed::Wake(feed::Wake { v: 1, watch: json(&text) }),
        "kept" => Feed::Kept(feed::Kept { text }),
        _ => return None,
    })
}

/// His routed words: `you → @{who} (answer to card #{id}) : {said}`, or
/// `you → @{who} : {said}`; an older hub wrote them in French (`toi →
/// @{who} (réponse à la carte #{id}) : …`).
fn route(text: &str) -> Option<Feed> {
    let (head, said) = text.split_once(" : ")?;
    let to = head.strip_prefix("you → @").or_else(|| head.strip_prefix("toi → @"))?;
    let (who, card) = match to.split_once(" (answer to card #").or_else(|| to.split_once(" (réponse à la carte #")) {
        Some((who, id)) => (who, Some(id.strip_suffix(')')?.parse().ok()?)),
        None if !to.contains(' ') => (to, None),
        None => return None,
    };
    Some(Feed::Route(feed::Route { who: who.into(), card, said: said.into(), msg: None, pick_n: None, pick: None }))
}

#[cfg(test)]
#[path = "legacy_tests.rs"]
mod tests;

/// An older text observation line (`  obs: …`) as its view; None for any
/// other line.
pub(crate) fn obs_line(line: &str) -> Option<Obs> {
    line.strip_prefix("  obs: ").map(obs)
}

/// An observation made of the text after its prefix.
type ObsOf = fn(String) -> Obs;

/// The runtime's older text observation (`  obs: <kind>: <text>`, before
/// issue 38's `obs <kind> :: <json>`), the `  obs: ` taken off.
pub fn obs(o: &str) -> Obs {
    let s = |t: &str| t.to_string();
    if o == "turn_started" {
        return Obs::TurnStarted;
    }
    if let Some(t) = o.strip_prefix("assistant: ") {
        return Obs::Assistant(s(t));
    }
    if o == "assistant:" {
        return Obs::Assistant(String::new());
    }
    if let Some(n) = o.strip_prefix("tool_started #") {
        return n.parse().map(Obs::ToolStarted).unwrap_or(Obs::Plumbing);
    }
    if let Some(rest) = o.strip_prefix("tool_finished #") {
        let Some((id, tail)) = rest.split_once(' ') else { return Obs::Plumbing };
        return id.parse().map(|id| Obs::ToolFinished { id, ok: tail == "ok" }).unwrap_or(Obs::Plumbing);
    }
    if o.starts_with("tool_result_committed") {
        return Obs::Plumbing;
    }
    let with: [(&str, ObsOf); 11] = [
        ("steering_received: ", Obs::SteeringReceived),
        ("steered: ", Obs::Steered),
        ("notification_received: ", Obs::NotificationReceived),
        ("notification_delivered: ", Obs::NotificationDelivered),
        ("provider_retry: ", Obs::ProviderRetry),
        ("harness_restarted: ", Obs::HarnessRestarted),
        ("candidate_discarded: ", Obs::CandidateDiscarded),
        ("context_compaction_failed: ", Obs::CompactionFailed),
        ("session_restored: ", Obs::SessionRestored),
        ("compaction_done: ", Obs::CompactionDone),
        ("usage: ", Obs::Usage),
    ];
    if o.starts_with("compaction_started #") {
        return Obs::CompactionStarted;
    }
    for (p, f) in with {
        if let Some(t) = o.strip_prefix(p) {
            return f(s(t));
        }
    }
    if o == "null_iteration" {
        return Obs::NullIteration;
    }
    if let Some(t) = o.strip_prefix("turn_done: ") {
        return Obs::TurnDone(match t {
            "completed" => TurnEnd::Completed,
            "interrupted" => TurnEnd::Interrupted { by: None },
            t => match t.strip_prefix("failed: ") {
                Some(why) => match why.strip_prefix("interrupted by ") {
                    Some(by) => TurnEnd::Interrupted { by: Some(s(by)) },
                    None => TurnEnd::Failed(s(why)),
                },
                None => TurnEnd::Other(s(t)),
            },
        });
    }
    if let Some((from, to)) = o.strip_prefix("model_swapped: ").and_then(|t| t.split_once(' ')) {
        if !from.is_empty() && !to.is_empty() && !to.contains(' ') {
            return Obs::ModelSwapped { from: s(from), to: s(to) };
        }
    }
    if let Some(t) = o.strip_prefix("turn_stalled: ") {
        return Obs::TurnStalled(s(t));
    }
    Obs::Other(s(o))
}
