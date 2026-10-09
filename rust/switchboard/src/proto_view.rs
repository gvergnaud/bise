//! The typed protocol's rows and live entries from the hub's own views
//! (bise desktop S3a; the shell is `daemon/proto.rs`): pure functions
//! from the client snapshot (`core::Hub::snapshot`, the `state` event)
//! to `bise_proto` rows, and the live fold of a subscribed thread (only
//! the entries that changed go out).

use bise_proto::rows::{question, Phase, Agent, Artifact, ArtifactPr, ArtifactVersion, Card, CardPage, AgentMode, Report, ReportKind, Status};
use bise_proto::diff::{DiffFile, DiffLine, DiffView, Hunk, LineKind};
use bise_proto::hub::{HubEv, Job as Followed, JobState};
use bise_proto::thread::lines::unescape;
use bise_proto::thread::words::one_line;
use bise_proto::thread::{self, Ctx, Entry, Line};
use bise_proto::Pos;
use serde_json::Value;
use std::collections::BTreeMap;
use std::path::Path;

pub mod approvals;
mod dev_servers;
mod diff_ask;
mod features;
pub mod heads;
mod merged;
mod models;
pub mod slash;
mod worktrees;
pub use dev_servers::{dev_servers, lsof_ports, server_name, Job};
pub use diff_ask::DiffAsk;
pub use features::{features, PlaceCard};
pub use models::models;
pub use merged::{landed_line, merged, LOG_FORMAT as MERGED_LOG_FORMAT};
pub use worktrees::{left_right, worktree_list, worktrees};

/// The entries of the newest lines a subscription keeps, so a live line
/// folds into the entry it belongs to (a tools entry growing).
pub const KEEP: usize = 8;

fn s(v: &Value, k: &str) -> String {
    v.get(k).and_then(Value::as_str).unwrap_or("").trim().to_string()
}

fn ms(v: &Value, k: &str) -> Option<u64> {
    v.get(k).and_then(Value::as_u64).filter(|m| *m > 0)
}

/// When each agent's status last changed, as the hub saw it (a view's
/// memory, lost on restart: then the first sight counts).
pub type Since = BTreeMap<String, (Status, u64)>;

/// The snapshot's agents as rows. `cards`: the snapshot's open cards (an
/// agent's `waits`); `user_kind`: the hub's (his cards); `vision`: the
/// hub's model catalog's rule (`bise_catalog::Catalog::vision`, K4), read
/// at each call so a /model change shows on the next agents event.
pub fn agents(
    snap: &Value,
    since: &mut Since,
    now: u64,
    user_kind: fn(&str) -> bool,
    vision: &dyn Fn(&str) -> Option<bool>,
    usage: &dyn Fn(&str) -> Option<bise_proto::rows::AgentUsage>,
    // its head (heads.rs): its newest entry's pos and its ended turns
    head: &dyn Fn(&str) -> (Option<bise_proto::Pos>, u64),
) -> Vec<Agent> {
    let list = |k: &str| snap.get(k).and_then(Value::as_array).cloned().unwrap_or_default();
    let cards = list("cards");
    let mut out = Vec::new();
    for a in list("agents") {
        let name = s(&a, "name");
        let (status, archived) = Status::of_hub(&s(&a, "status"));
        let working = status.working();
        let first = ms(&a, "report_ms").filter(|_| !working).or(ms(&a, "created_ms")).unwrap_or(now);
        let at = match since.get(&name) {
            Some((st, at)) if *st == status => *at,
            Some(_) => now,
            None => first,
        };
        since.insert(name.clone(), (status, at));
        let title = [s(&a, "note"), s(&a, "role"), s(&a, "report"), s(&a, "objective")].into_iter().find(|t| !t.is_empty()).unwrap_or_default();
        let mode = match s(&a, "mode").as_str() {
            "worktree" => Some(AgentMode::Worktree),
            "shared" => Some(AgentMode::Shared),
            "" => None,
            _ => Some(AgentMode::Unknown),
        };
        let worktree = mode == Some(AgentMode::Worktree);
        let report = Some(s(&a, "report")).filter(|r| !r.is_empty()).map(|text| Report {
            kind: match status {
                Status::Done => ReportKind::Done,
                Status::Failed => ReportKind::Failed,
                Status::Blocked => ReportKind::Blocked,
                _ => ReportKind::Progress,
            },
            text,
            at_ms: ms(&a, "report_ms").unwrap_or(0),
        });
        let waits = cards.iter().filter(|c| s(c, "agent") == name && his(&s(c, "kind"), user_kind)).count() as u32;
        out.push(Agent {
            main: a.get("main").and_then(Value::as_bool).unwrap_or(false),
            status,
            phase: Phase::of_hub(&s(&a, "status")),
            archived,
            title: one_line(&title, 120),
            purpose: one_line(&s(&a, "objective"), 200),
            since_ms: at,
            waits,
            parent: Some(s(&a, "parent")).filter(|p| !p.is_empty()),
            // its worktree's branch as released; the shared checkout's
            // branch apart (P4b-fix, the wire rule)
            branch: Some(s(&a, "branch")).filter(|b| worktree && !b.is_empty()),
            checkout_branch: Some(s(&a, "branch")).filter(|b| !worktree && !b.is_empty()),
            worktree: Some(s(&a, "path")).filter(|p| worktree && !p.is_empty()),
            turn_ms: a.get("turn_ms").and_then(Value::as_u64).filter(|_| working),
            report,
            // his queued inputs (sb-core holds them, amb-hub 7dcb56ab)
            queued: a.get("queued_inputs").cloned().and_then(|q| serde_json::from_value(q).ok()).unwrap_or_default(),
            // where its thread is on disk, for stream C's readers (S2 step 3)
            dir: Some(s(&a, "dir")).filter(|d| !d.is_empty()).unwrap_or_else(|| name.clone()),
            aliases: a.get("aliases").cloned().and_then(|x| serde_json::from_value(x).ok()).unwrap_or_default(),
            // K4: its model and whether it reads images (the composer
            // refuses an image when false)
            vision: Some(s(&a, "model")).filter(|m| !m.is_empty()).and_then(|m| vision(&m)),
            model: Some(s(&a, "model")).filter(|m| !m.is_empty()),
            effort: Some(s(&a, "effort")).filter(|e| !e.is_empty()),
            // S13: its context after its last call (the hub's live usage)
            usage: usage(&name),
            // R9/S3: model.rs's waiting_on, the field the TUI's panel reads
            waiting_on: bise_proto::rows::WaitingOn::of_word(&s(&a, "waiting_on")),
            // P4b: the terminal's facts, each the snapshot's own field
            mode,
            path: s(&a, "path"),
            objective: s(&a, "objective"),
            note: s(&a, "note"),
            role: s(&a, "role"),
            msgs_queued: a.get("queued").and_then(Value::as_u64).unwrap_or(0) as u32,
            inbox: a.get("inbox").and_then(Value::as_u64).unwrap_or(0) as u32,
            created_ms: ms(&a, "created_ms"),
            place: Some(s(&a, "place")).filter(|p| !p.is_empty()),
            place_id: s(&a, "place_id"),
            efforts: a.get("efforts").cloned().and_then(|e| serde_json::from_value(e).ok()).unwrap_or_default(),
            changes: a.get("changes").cloned().and_then(|c| serde_json::from_value(c).ok()),
            last_pos: head(&name).0,
            turns: head(&name).1,
            name,
        });
    }
    since.retain(|n, _| out.iter().any(|a| &a.name == n));
    out
}

/// ⌘K and the scheduled screen (architect m_11874): the live timers as
/// rows, from the hub's typed timer state (never its lines); the
/// how-often words are bise-proto's, the TUI's (`every_words`). The one
/// builder: the `scheduled` event and view.json's index both call it.
pub fn scheduled(timers: &crate::every::Timers) -> Vec<bise_proto::rows::ScheduledTask> {
    timers.map.values().map(task_row).collect()
}

/// P4c-4a: the timers that ended in the last week, oldest first, as rows
/// (the scheduled screen's tab; `HubEv::Scheduled.ended`): the hub's
/// state's (`Timers::state`) with how they ended (`every::end_of`).
pub fn scheduled_ended(timers: &crate::every::Timers, now: u64) -> Vec<bise_proto::rows::ScheduledTask> {
    use bise_proto::rows::{ScheduledEnd, WaitingOn};
    timers
        .ended
        .iter()
        .filter(|e| e.ended_ms + crate::every::ENDED_SHOWN_MS > now)
        .map(|e| {
            let (end, by) = crate::every::end_of(&e.why);
            let stopped_by = match by.as_str() {
                "" => None,
                "user" => Some(WaitingOn::You),
                name => Some(WaitingOn::Agent { name: name.to_string() }),
            };
            bise_proto::rows::ScheduledTask { ended_ms: Some(e.ended_ms), end: ScheduledEnd::of_word(end), stopped_by, ..task_row(&e.timer) }
        })
        .collect()
}

fn task_row(t: &crate::every::Timer) -> bise_proto::rows::ScheduledTask {
    bise_proto::rows::ScheduledTask {
        id: t.id,
        agent: t.agent.clone(),
        by: t.by.clone(),
        words: t.text.clone(),
        name: t.title(),
        every: bise_proto::thread::scheduled::every_words(&t.sched.label(), t.times),
        times: t.times,
        done: t.fired,
        next_ms: Some(t.next_ms).filter(|ms| *ms > 0),
        until_ms: t.until_ms,
        page: t.page.clone().filter(|p| !p.is_empty()),
        label: t.sched.label(),
        last_ms: Some(t.last_ms).filter(|ms| *ms > 0),
        runs: t.runs.clone(),
        ended_ms: None,
        end: None,
        stopped_by: None,
    }
}

/// P4c-4a: the snapshot's `places` (`place::views`, the worktrees the
/// terminal's panel draws) as rows, each PR by the one builder
/// (`forge::news::pr_row`, `/prs`'s) with its place's agents.
pub fn places(snap: &Value) -> Vec<bise_proto::rows::Place> {
    let views: Vec<crate::place::PlaceView> = snap.get("places").cloned().and_then(|p| serde_json::from_value(p).ok()).unwrap_or_default();
    views
        .into_iter()
        .map(|p| bise_proto::rows::Place {
            pr: p.pr.as_ref().map(|pr| crate::forge::news::pr_row(pr, &p.agents)),
            id: p.id,
            branch: p.branch,
            agents: p.agents,
            lid: p.lid,
            feature: p.feature,
            trying: p.trying,
        })
        .collect()
}

/// S10: the followed agents of a hub snapshot, each a running job: its
/// last progress (else its objective), `--step n/m`, since its last
/// report (else its creation). A follow ends with its job (job_end), so
/// every row runs.
pub fn jobs(snap: &Value) -> Vec<Followed> {
    let list = snap.get("agents").and_then(Value::as_array).cloned().unwrap_or_default();
    let n = |a: &Value, k: &str| a.get(k).and_then(Value::as_u64).and_then(|x| u32::try_from(x).ok());
    list.iter()
        .filter(|a| a.get("follow").and_then(Value::as_bool).unwrap_or(false))
        .map(|a| {
            let progress = Some(s(a, "report")).filter(|r| !r.is_empty() && s(a, "report_kind") == "progress");
            Followed {
                agent: s(a, "name"),
                label: one_line(&progress.unwrap_or_else(|| s(a, "objective")), 120),
                step: n(a, "step"),
                of: n(a, "of"),
                state: JobState::Running,
                since_ms: ms(a, "report_ms").or(ms(a, "created_ms")).unwrap_or(0),
            }
        })
        .collect()
}

/// His cards: the user's kinds, bise's own bookkeeping (a `drop`) left
/// to the TUI (pm's C fail 36).
fn his(kind: &str, user_kind: fn(&str) -> bool) -> bool {
    user_kind(kind) && kind != "drop"
}

/// The snapshot's open cards as rows, in its order: his (`HubEv::Cards`'
/// `cards`), then the others (`others`, P4c-4a: bise's own, a drop, a
/// done, a blocked..., which the terminal's inbox draws too). Each with
/// the snapshot's words (`text`, its `question` and `options` parsed
/// from them), note, place, PR, link and the message it answers.
pub fn cards(snap: &Value, project: &str, now: u64, user_kind: fn(&str) -> bool) -> (Vec<Card>, Vec<Card>) {
    let list = snap.get("cards").and_then(Value::as_array).cloned().unwrap_or_default();
    let opt = |c: &Value, k: &str| Some(s(c, k)).filter(|x| !x.is_empty());
    let n = |c: &Value, k: &str| c.get(k).and_then(Value::as_u64);
    list.iter()
        .map(|c| {
            let kind = s(c, "kind");
            let text = c.get("text").and_then(Value::as_str).unwrap_or("").to_string();
            let (q, options) = question(&text);
            let page = c.get("page").filter(|p| p.is_object()).map(|p| CardPage {
                id: s(p, "id"),
                url: s(p, "url"),
                block: Some(s(p, "block")).filter(|b| !b.is_empty()),
                item: Some(s(p, "item")).filter(|i| !i.is_empty()),
            });
            Card {
                id: n(c, "id").unwrap_or(0),
                project: project.to_string(),
                urgent: kind == "confirm",
                approval: Card::approval_kind(&kind),
                rank: Some(bise_proto::rows::card_rank(&kind)),
                agent: s(c, "agent"),
                question: q,
                options,
                since_ms: now.saturating_sub(n(c, "age_ms").unwrap_or(0)),
                page,
                // V14: the signin card's stopped agents (core.rs signin_waiting)
                waiting: c.get("waiting").and_then(Value::as_array).map(|a| a.iter().filter_map(Value::as_str).map(str::to_string).collect()),
                note: opt(c, "note"),
                place: opt(c, "place"),
                pr: n(c, "pr"),
                link: opt(c, "link"),
                for_msg: n(c, "for_msg"),
                text,
                kind,
            }
        })
        .partition(|c| his(&c.kind, user_kind))
}

/// The art store's `artifacts` event (daemon/art.rs `artifacts_ev`:
/// rows + `seen_ms`) as rows. `url`: a page's the page server gives
/// (`page_url`), else its link or its file (its kept copy first); `new`:
/// not his own, changed since he last looked; the rest of each row as
/// the store gave it (the terminal's /artifacts reads it all).
pub fn artifacts(ev: &Value, page_url: impl Fn(&str) -> Option<String>) -> Vec<Artifact> {
    let seen = ev.get("seen_ms").and_then(Value::as_u64).unwrap_or(0);
    let rows = ev.get("rows").and_then(Value::as_array).cloned().unwrap_or_default();
    rows.iter()
        .map(|r| {
            let (id, kind) = (s(r, "id"), s(r, "kind"));
            let at_ms = r.get("ts_ms").and_then(Value::as_u64).unwrap_or(0);
            let url = match page_url(&id).filter(|_| s(r, "by") == "page") {
                Some(u) => u,
                None => Some(s(r, "copy")).filter(|c| !c.is_empty() && !s(r, "target").contains("://")).unwrap_or_else(|| s(r, "target")),
            };
            let target = s(r, "target");
            let path = Some(target.clone()).filter(|t| s(r, "by") != "page" && !crate::artifacts::is_link(t) && Path::new(t).is_absolute());
            let versions = r.get("versions").and_then(Value::as_array).map_or_else(Vec::new, |vs| {
                vs.iter()
                    .map(|v| ArtifactVersion {
                        v: v.get("v").and_then(Value::as_u64).unwrap_or(0) as u32,
                        at_ms: v.get("ts_ms").and_then(Value::as_u64).unwrap_or(0),
                        note: Some(s(v, "note")).filter(|n| !n.is_empty()),
                        target: s(v, "target"),
                        copy: Some(s(v, "copy")).filter(|c| !c.is_empty()),
                    })
                    .collect()
            });
            let pr = r.get("pr").filter(|p| p.is_object()).map(|p| ArtifactPr { repo: s(p, "repo"), number: p.get("number").and_then(Value::as_u64).unwrap_or(0) });
            Artifact {
                path,
                versions,
                title: s(r, "title"),
                agent: s(r, "agent"),
                version: r.get("v").and_then(Value::as_u64).unwrap_or(0) as u32,
                new: s(r, "by") != "you" && at_ms > seen,
                at_ms,
                url,
                kind,
                id,
                by: s(r, "by"),
                archived: r.get("archived").and_then(Value::as_bool).unwrap_or(false),
                created_ms: r.get("created_ms").and_then(Value::as_u64),
                copy: Some(s(r, "copy")).filter(|c| !c.is_empty()),
                gone: r.get("gone").and_then(Value::as_bool).unwrap_or(false),
                detail: s(r, "detail"),
                pr,
                keys: r.get("keys").and_then(Value::as_array).map_or_else(Vec::new, |k| k.iter().filter_map(|x| x.as_str().map(String::from)).collect()),
                target,
            }
        })
        .collect()
}

/// The hub's `diff` answer (daemon/art.rs `diff_answer`: files of
/// `diff::file_json`) as the typed `diff`, or its error. A file cut for
/// its length keeps what it has and says `truncated`, as a binary file
/// (no text to show); a generated file keeps its lines under the same cap
/// (`generated` is a display flag: the client folds it, architect
/// m_13436); `image` and each hunk's `head` as `file_json` says them.
/// A commit's sha as a client may name one: 4 to 40 hex digits (never an
/// option or a range for git).
pub fn is_sha(c: &str) -> bool {
    (4..=40).contains(&c.len()) && c.chars().all(|ch| ch.is_ascii_hexdigit())
}

pub fn diff(ev: &Value, project: &str, agent: &str) -> HubEv {
    if let Some(e) = ev.get("error").and_then(Value::as_str) {
        return HubEv::Error { project: Some(project.into()), cmd: Some("diff".into()), text: e.into(), cid: None, reason: None, kind: None };
    }
    let files = ev.get("files").and_then(Value::as_array).cloned().unwrap_or_default();
    let flag = |f: &Value, k: &str| f.get(k).and_then(Value::as_bool).unwrap_or(false);
    let files = files
        .iter()
        .map(|f| {
            let hunks: Vec<Hunk> = f.get("hunks").and_then(Value::as_array).into_iter().flatten().map(hunk).collect();
            DiffFile {
                path: s(f, "path"),
                status: match s(f, "status").as_str() {
                    "A" => "added",
                    "D" => "deleted",
                    "R" => "renamed",
                    _ => "modified",
                }
                .into(),
                add: f.get("add").and_then(Value::as_u64).unwrap_or(0) as u32,
                del: f.get("del").and_then(Value::as_u64).unwrap_or(0) as u32,
                truncated: flag(f, "cut") || flag(f, "binary"),
                from: Some(s(f, "old_path")).filter(|p| !p.is_empty() && *p != s(f, "path")),
                binary: flag(f, "binary"),
                generated: flag(f, "generated"),
                image: flag(f, "image"),
                size: f.get("size").and_then(Value::as_u64),
                hunks,
                note: None,
                abs: Some(s(f, "abs")).filter(|a| !a.is_empty()),
            }
        })
        .collect();
    HubEv::Diff {
        project: project.into(),
        agent: agent.into(),
        base: s(ev, "base"),
        head: Some(s(ev, "branch")).filter(|b| !b.is_empty()),
        files,
        result: None,
        merged: None,
        commit: None,
        note: Some(s(ev, "note")).filter(|n| !n.is_empty()),
        view: Box::new(DiffView {
            title: Some(s(ev, "title")).filter(|t| !t.is_empty()),
            req: ev.get("req").and_then(Value::as_u64),
            commits: ev.get("commits").and_then(Value::as_u64),
            working: flag(ev, "working"),
            uncommitted: flag(ev, "uncommitted"),
            landed_ms: ev.get("landed_ms").and_then(Value::as_u64),
            gone: flag(ev, "gone"),
        }),
    }
}

/// One hunk of `diff::file_json` (`old`/`new` its first lines, `lines`
/// prefixed ' ', '-', '+'), each line numbered on its side.
fn hunk(h: &Value) -> Hunk {
    let (mut o, mut n) = (h.get("old").and_then(Value::as_u64).unwrap_or(0) as u32, h.get("new").and_then(Value::as_u64).unwrap_or(0) as u32);
    let head = s(h, "head");
    let header = format!("@@ -{o} +{n} @@ {head}").trim_end().to_string();
    let mut lines = Vec::new();
    for l in h.get("lines").and_then(Value::as_array).into_iter().flatten().filter_map(Value::as_str) {
        let text = l.get(1..).unwrap_or("").to_string();
        let line = match l.chars().next() {
            Some('+') => {
                n += 1;
                DiffLine { kind: LineKind::Add, old: None, new: Some(n - 1), text }
            }
            Some('-') => {
                o += 1;
                DiffLine { kind: LineKind::Del, old: Some(o - 1), new: None, text }
            }
            Some(' ') => {
                o += 1;
                n += 1;
                DiffLine { kind: LineKind::Ctx, old: Some(o - 1), new: Some(n - 1), text }
            }
            // `\ No newline at end of file` and the like
            _ => continue,
        };
        lines.push(line);
    }
    Hunk { header, lines, head: Some(head).filter(|h| !h.is_empty()) }
}

/// An agent's current step from one feed line: its tool intent, "" at
/// its turn's end; None: the line says nothing of it.
pub fn step_of(line: &str) -> Option<String> {
    if let Some(r) = line.strip_prefix("tool_intent #") {
        let text = r.split_once(" : ").map_or("", |(_, t)| t);
        return Some(one_line(&unescape(text), 80));
    }
    line.starts_with("  obs: turn_done").then(String::new)
}

/// A subscribed thread's newest lines and what was sent of each entry.
#[derive(Default)]
pub struct Live {
    lines: Vec<Line>,
    sent: BTreeMap<Pos, String>,
}

impl Live {
    /// After its page: the lines of the page's newest entries are kept,
    /// the page's entries count as sent.
    pub fn start(lines: Vec<Line>, page: &[Entry]) -> Live {
        let mut l = Live { lines, sent: BTreeMap::new() };
        for e in page {
            l.sent.insert(e.pos, json(e));
        }
        l.trim(page);
        l
    }

    /// One live line: the entries it made or changed (a line already
    /// seen, a replay after a reconnection, changes nothing).
    pub fn push(&mut self, line: Line, ctx: &Ctx) -> Vec<Entry> {
        if self.lines.last().is_some_and(|l| l.0 >= line.0) {
            return Vec::new();
        }
        self.lines.push(line);
        self.refold(ctx)
    }

    /// Its newest entry's pos (the last one sent): `hub/agents`'
    /// `last_pos` without a second fold (heads.rs).
    pub fn last_pos(&self) -> Option<Pos> {
        self.sent.keys().next_back().copied()
    }

    /// The fold again (a card answered, a page published): what changed.
    pub fn refold(&mut self, ctx: &Ctx) -> Vec<Entry> {
        let entries = thread::fold(&self.lines, ctx);
        let mut out = Vec::new();
        for e in &entries {
            let j = json(e);
            if self.sent.get(&e.pos) != Some(&j) {
                self.sent.insert(e.pos, j);
                out.push(e.clone());
            }
        }
        self.trim(&entries);
        out
    }

    fn trim(&mut self, entries: &[Entry]) {
        if entries.len() > KEEP {
            let cut = entries[entries.len() - KEEP].pos;
            self.lines.retain(|l| l.0 >= cut);
            self.sent.retain(|p, _| *p >= cut);
        }
    }
}

fn json(e: &Entry) -> String {
    serde_json::to_string(e).unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn uk(k: &str) -> bool {
        matches!(k, "question" | "drop" | "confirm")
    }

    /// Law (architect m_11874): the `scheduled` row the hub builds from a
    /// timer and the TUI's Task::of on the line the hub writes for it say
    /// the same id, agent, words, how often, times and runs so far.
    #[test]
    fn the_scheduled_rows_and_the_tuis_lines_agree() {
        use crate::every::{Sched, Timer, Timers};
        use bise_proto::thread::scheduled::Task;
        let t = |id: u64, sched: Sched, times: Option<u64>, until_ms: Option<u64>, fired: u64| Timer {
            id,
            agent: "perf".into(),
            by: "main".into(),
            text: format!("check #{id}"),
            sched,
            next_ms: 1_790_000_120_000,
            until_ms,
            times,
            fired,
            page: None,
            last_ms: 0,
            runs: vec![],
            name: if id == 1 { "perf check".into() } else { String::new() },
        };
        let mut timers = Timers::default();
        for x in [
            t(1, Sched::Every(120_000), None, None, 0),
            t(2, Sched::Daily(450), None, None, 3),
            t(3, Sched::Every(600_000), Some(1), None, 0),
            t(4, Sched::Every(60_000), Some(6), None, 2),
            t(5, Sched::Every(300_000), None, Some(1_790_100_000_000), 1),
        ] {
            timers.map.insert(x.id, x);
        }
        let rows = scheduled(&timers);
        assert_eq!(rows.len(), 5);
        for r in &rows {
            let mut line = timers.map[&r.id].json();
            line["ev"] = json!("set");
            let task = Task::of(&line).unwrap();
            assert_eq!((r.id, &r.agent, &r.words, &r.every, r.times, r.done), (task.id, &task.agent, &task.text, &task.when(), task.times, task.fired), "#{}", r.id);
            // the name: the row's and the TUI's title say the same (sched-names)
            assert_eq!(r.name, task.title(), "#{}", r.id);
        }
        // a name never repeats its agent (designer m_15602): perf's "perf check" reads "check"
        assert_eq!((rows[0].name.as_str(), rows[1].name.as_str()), ("check", "check #2"), "a name, else the fallback");
        let every: Vec<&str> = rows.iter().map(|r| r.every.as_str()).collect();
        assert_eq!(every, ["every 2m", "every day 07:30", "once", "every 1m", "every 5m"]);
    }

    fn snap() -> Value {
        json!({"ev": "state",
            "agents": [
                {"name": "main", "main": true, "status": "idle", "objective": "", "created_ms": 5},
                {"name": "perf", "main": false, "status": "working", "objective": "make the e2e fast\nmore", "note": "",
                 "role": "profiling the hub", "mode": "worktree", "branch": "sb/perf", "path": "/w/perf", "parent": "main",
                 "turn_ms": 4200, "created_ms": 7, "queued": 1,
                 "queued_inputs": [{"id": 3, "text": "then the cold run", "created_ms": 8}]},
                {"name": "old", "main": false, "status": "archived", "objective": "an old fix", "report": "fixed in 0.4",
                 "report_ms": 9, "mode": "shared", "branch": "main", "path": "/w"},
            ],
            "cards": [
                {"id": 9, "kind": "question", "agent": "perf", "text": "which bench?\n1. cold\n2. warm", "age_ms": 1000},
                {"id": 10, "kind": "confirm", "agent": "perf", "text": "run it?", "age_ms": 0,
                 "page": {"id": "p", "url": "http://x/p/p", "block": "q1"}},
                {"id": 11, "kind": "drop", "agent": "old", "text": "archive old?", "age_ms": 0},
                {"id": 12, "kind": "done", "agent": "perf", "text": "perf is done", "age_ms": 0},
            ]})
    }

    /// K4 (architect m_10064): an agent's row carries its model and
    /// whether it reads images from the hub's catalog, read fresh, so a
    /// /model change shows on the next agents event.
    #[test]
    fn an_agent_row_says_its_model_and_whether_it_reads_images() {
        let cat = bise_catalog::Catalog::builtin();
        let vision = |m: &str| cat.vision(m);
        let mut since = Since::new();
        let mut s = snap();
        s["agents"][1]["model"] = json!("mistral/codestral-latest");
        s["agents"][2]["model"] = json!("ollama/llava");
        let a = agents(&s, &mut since, 100, uk, &vision, &|_| None, &|_| (None, 0));
        assert_eq!((a[1].model.as_deref(), a[1].vision), (Some("mistral/codestral-latest"), Some(false)), "listed, no vision");
        assert_eq!((a[2].model.as_deref(), a[2].vision), (Some("ollama/llava"), None), "unlisted: never a guess");
        assert_eq!((a[0].model.as_deref(), a[0].vision), (None, None), "no model in the snapshot");
        // his /model switch: the next rows follow
        s["agents"][1]["model"] = json!("anthropic/claude-haiku-4-5");
        assert_eq!(agents(&s, &mut since, 200, uk, &vision, &|_| None, &|_| (None, 0))[1].vision, Some(true));
    }

    #[test]
    fn the_snapshot_becomes_rows() {
        let mut since = Since::new();
        let a = agents(&snap(), &mut since, 100, uk, &|_| None, &|_| None, &|_| (None, 0));
        assert_eq!(a.len(), 3);
        let p = &a[1];
        assert_eq!((p.status, p.title.as_str(), p.purpose.as_str(), p.waits), (Status::Working, "profiling the hub", "make the e2e fast", 2));
        assert_eq!((p.branch.as_deref(), p.worktree.as_deref(), p.turn_ms, p.since_ms), (Some("sb/perf"), Some("/w/perf"), Some(4200), 7));
        let o = &a[2];
        // branch keeps its released meaning (a worktree's); the shared
        // checkout's branch is checkout_branch (P4b-fix, the wire rule)
        assert_eq!((o.archived, o.status, o.branch.as_deref(), o.checkout_branch.as_deref(), o.since_ms), (true, Status::Done, None, Some("main"), 9));
        assert_eq!((p.checkout_branch.as_deref(), p.phase), (None, None));
        assert_eq!((o.mode, o.worktree.as_deref(), o.path.as_str()), (Some(AgentMode::Shared), None, "/w"));
        assert_eq!((p.mode, p.path.as_str(), p.objective.as_str(), p.role.as_str(), p.msgs_queued, p.created_ms), (Some(AgentMode::Worktree), "/w/perf", "make the e2e fast\nmore", "profiling the hub", 1, Some(7)));
        assert_eq!((a[0].dir.as_str(), p.dir.as_str()), ("main", "perf"), "no dir in the snapshot: its name");
        assert_eq!(p.last_pos, None, "no head given");
        assert_eq!(o.report.as_ref().map(|r| r.kind), Some(ReportKind::Done));
        assert_eq!(p.queued.iter().map(|q| (q.id, q.text.as_str())).collect::<Vec<_>>(), [(3, "then the cold run")]);
        assert!(o.queued.is_empty());
        // a status change moves since; the same keeps it
        let mut s2 = snap();
        s2["agents"][1]["status"] = json!("idle");
        assert_eq!(agents(&s2, &mut since, 200, uk, &|_| None, &|_| None, &|_| (None, 0))[1].since_ms, 200);
        assert_eq!(agents(&s2, &mut since, 300, uk, &|_| None, &|_| None, &|_| (None, 0))[1].since_ms, 200);
        let (c, others) = cards(&snap(), "acme", 5_000, uk);
        assert_eq!(c.iter().map(|c| c.id).collect::<Vec<_>>(), [9, 10], "his kinds, no drop");
        // P4c-4a: bise's own (a drop, a done) are the others, in order
        assert_eq!(others.iter().map(|c| (c.id, c.kind.as_str())).collect::<Vec<_>>(), [(11, "drop"), (12, "done")]);
        assert_eq!(c[0].text, "which bench?\n1. cold\n2. warm");
        assert_eq!((c[0].question.as_str(), c[0].options.len(), c[0].since_ms, c[0].urgent), ("which bench?", 2, 4_000, false));
        assert!(c[1].urgent && c[1].page.as_ref().is_some_and(|p| p.block.as_deref() == Some("q1")));
    }

    #[test]
    fn the_art_store_rows_become_artifacts() {
        let ev = json!({"ev": "artifacts", "seen_ms": 100, "rows": [
            {"id": "perf-notes", "kind": "page", "title": "Perf notes", "agent": "perf", "by": "page", "ts_ms": 150, "v": 2,
             "target": "/st/pages/perf-notes", "copy": null},
            {"id": "deck", "kind": "file", "title": "Deck", "agent": "main", "by": "agent", "ts_ms": 90, "v": 2,
             "target": "/w/deck.pdf", "copy": "/st/art/deck/1.pdf", "versions": [
                {"v": 1, "ts_ms": 80, "target": "/w/deck.pdf", "copy": "/st/art/deck/1.pdf", "note": ""},
                {"v": 2, "ts_ms": 90, "target": "/w/deck.pdf", "copy": null, "note": "no copy: too big"}]},
            {"id": "site", "kind": "site", "title": "Site", "agent": "web", "by": "agent", "ts_ms": 200, "v": 3,
             "target": "https://x.dev", "copy": "/st/art/site/3"},
            {"id": "mine", "kind": "file", "title": "Mine", "agent": "main", "by": "you", "ts_ms": 300, "v": 1,
             "target": "/w/m.txt", "copy": null}]});
        let a = artifacts(&ev, |id| (id == "perf-notes").then(|| "http://127.0.0.1:1/p/perf-notes".to_string()));
        let row = |i: usize| (a[i].id.as_str(), a[i].url.as_str(), a[i].version, a[i].new);
        assert_eq!(row(0), ("perf-notes", "http://127.0.0.1:1/p/perf-notes", 2, true), "a page: its server url");
        assert_eq!(row(1), ("deck", "/st/art/deck/1.pdf", 2, false), "a file: its kept copy, seen before");
        assert_eq!(row(2), ("site", "https://x.dev", 3, true), "a link: the link");
        assert_eq!(row(3), ("mine", "/w/m.txt", 1, false), "his own is never new");
        let paths: Vec<Option<&str>> = a.iter().map(|x| x.path.as_deref()).collect();
        assert_eq!(paths, [None, Some("/w/deck.pdf"), None, Some("/w/m.txt")], "a path for a file only, never a page or a link");
        let notes: Vec<(u32, u64, Option<&str>)> = a[1].versions.iter().map(|x| (x.v, x.at_ms, x.note.as_deref())).collect();
        assert_eq!(notes, [(1, 80, None), (2, 90, Some("no copy: too big"))], "the store's versions, oldest first, its words");
        assert!(a[0].versions.is_empty(), "a row without versions has none");
    }

    #[test]
    fn the_hubs_diff_becomes_the_typed_diff() {
        let ev = json!({"ev": "diff", "base": "main", "branch": "sb/perf", "files": [
            {"path": "src/q.rs", "status": "M", "add": 2, "del": 1, "binary": false, "generated": false, "cut": false,
             "hunks": [{"old": 10, "new": 10, "head": "fn slow()", "lines": [" let r = q();", "-r.sort();", "+r.sort_unstable();", "+r.dedup();", "\\ No newline at end of file"]}]},
            {"path": "Cargo.lock", "status": "M", "add": 9, "del": 9, "binary": false, "generated": true, "cut": false,
             "hunks": [{"old": 1, "new": 1, "head": "", "lines": ["-a", "+b"]}]},
            {"path": "big.rs", "status": "A", "add": 9000, "del": 0, "binary": false, "generated": false, "cut": true, "hunks": []},
            {"path": "logo.png", "status": "A", "add": 0, "del": 0, "binary": true, "image": true, "generated": false, "cut": false, "hunks": [], "size": 2100000},
            {"path": "src/codec.rs", "old_path": "src/decode.rs", "status": "R", "add": 0, "del": 0, "binary": false, "generated": false, "cut": false, "hunks": []}]});
        let HubEv::Diff { files, head, base, .. } = diff(&ev, "acme", "perf") else { panic!("a diff") };
        assert_eq!((base.as_str(), head.as_deref()), ("main", Some("sb/perf")));
        let h = &files[0].hunks[0];
        assert_eq!((h.header.as_str(), h.head.as_deref()), ("@@ -10 +10 @@ fn slow()", Some("fn slow()")));
        let l = |i: usize| (h.lines[i].kind, h.lines[i].old, h.lines[i].new, h.lines[i].text.as_str());
        assert_eq!(l(0), (LineKind::Ctx, Some(10), Some(10), "let r = q();"));
        assert_eq!(l(1), (LineKind::Del, Some(11), None, "r.sort();"));
        assert_eq!(l(2), (LineKind::Add, None, Some(11), "r.sort_unstable();"));
        assert_eq!(l(3), (LineKind::Add, None, Some(12), "r.dedup();"));
        assert_eq!(h.lines.len(), 4, "no-newline markers dropped");
        assert_eq!((files[0].status.as_str(), files[0].truncated), ("modified", false));
        assert!(!files[1].truncated && files[1].hunks.len() == 1, "a generated file: its lines, the client folds it");
        assert_eq!(files[1].hunks[0].head, None, "no function context: no head");
        assert!(files[2].truncated && files[2].status == "added", "too long: said cut");
        assert!(files[3].truncated, "binary: nothing to show, said so");
        // amb-web m_8827: each cut says why, a rename its old path
        assert_eq!((files[1].generated, files[1].binary, files[2].generated, files[2].binary), (true, false, false, false), "a plain cut is neither");
        assert_eq!((files[3].binary, files[3].image, files[3].size), (true, true, Some(2_100_000)));
        assert!(!files[0].image);
        assert_eq!((files[4].status.as_str(), files[4].from.as_deref()), ("renamed", Some("src/decode.rs")));
        assert_eq!(files[0].from, None, "not renamed: no from");
        assert!(is_sha("e0f3df5") && is_sha(&"a".repeat(40)));
        assert!(!is_sha("abc") && !is_sha("--output=x") && !is_sha("a..b") && !is_sha("HEAD") && !is_sha(&"a".repeat(41)));
        let e = diff(&json!({"error": "no agent x", "files": []}), "acme", "x");
        assert!(matches!(e, HubEv::Error { cmd: Some(ref c), .. } if c == "diff"));
    }

    #[test]
    fn a_live_thread_sends_only_what_changed() {
        let none = |_: &str| None;
        let open: [u64; 0] = [];
        let ctx = Ctx { open_cards: &open, page: &none, provider: &|i: &str, _: &str| i.to_string(), width: &unicode_width::UnicodeWidthStr::width, offset: &|_| 0, attached: &crate::attached::split };
        let l = |pos: u64, line: &str| (pos, pos, line.to_string());
        let first = vec![l(1, "sb you : go"), l(2, "  obs: assistant: on it")];
        let page = thread::fold(&first, &ctx);
        let mut live = Live::start(first, &page);
        assert!(live.push(l(2, "  obs: assistant: on it"), &ctx).is_empty(), "a replay");
        // a tool row exists from its tool_started (G3)
        let e = live.push(l(3, "  obs: tool_started #1"), &ctx);
        assert_eq!((e.len(), e[0].pos), (1, 3));
        let e = live.push(l(4, "tool #1 bash : ls"), &ctx);
        assert_eq!((e.len(), e[0].pos), (1, 3));
        let e = live.push(l(5, "tool_intent #1 : listing"), &ctx);
        assert_eq!((e.len(), e[0].pos, e[0].tools.as_ref().unwrap().items[0].text.as_str()), (1, 3, "listing"));
        live.push(l(6, "  obs: tool_started #2"), &ctx);
        let e = live.push(l(7, "tool #2 read_file : a"), &ctx);
        assert_eq!((e[0].pos, e[0].tools.as_ref().unwrap().count), (3, 2), "the same entry grows");
        assert_eq!(step_of("tool_intent #2 : reading a"), Some("reading a".into()));
        assert_eq!(step_of("  obs: turn_done: completed"), Some(String::new()));
        assert_eq!(step_of("tool #2 x : y"), None);
    }
}
