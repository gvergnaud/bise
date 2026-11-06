//! The typed protocol's rows and live entries from the hub's own views
//! (bise desktop S3a; the shell is `daemon/proto.rs`): pure functions
//! from the hub's typed snapshot (`crate::snapshot::Snapshot`, read by
//! field) to `bise_proto` rows, and the live fold of a subscribed thread (only
//! the entries that changed go out).

use bise_proto::rows::{question, Phase, Agent, ArchivedAgent, Artifact, ArtifactPr, ArtifactVersion, Card, CardPage, AgentMode, Report, ReportKind, Status};
use bise_proto::diff::{DiffFile, DiffLine, DiffView, Hunk, LineKind};
use bise_proto::hub::{HubEv, Job as Followed, JobState};
use bise_proto::thread::lines::unescape;
use bise_proto::thread::words::one_line;
use bise_proto::thread::{self, Ctx, Entry, Line};
use bise_proto::Pos;
use crate::snapshot::{AgentRow, Snapshot};
use serde_json::Value;
use std::collections::BTreeMap;
use std::path::Path;

pub mod approvals;
mod diff_ask;
mod features;
mod growth;
pub mod heads;
mod merged;
mod models;
pub mod processes;
pub mod rank;
pub mod slash;
mod worktrees;
pub use diff_ask::DiffAsk;
pub use features::{features, PlaceCard};
pub use growth::growth_rows;
pub use models::models;
pub use merged::{landed_line, merged, LOG_FORMAT as MERGED_LOG_FORMAT};
pub use worktrees::{left_right, worktree_list, worktrees};

/// The entries of the newest lines a subscription keeps, so a live line
/// folds into the entry it belongs to (a tools entry growing).
pub const KEEP: usize = 8;

fn s(v: &Value, k: &str) -> String {
    v.get(k).and_then(Value::as_str).unwrap_or("").trim().to_string()
}


/// When each agent's status last changed, as the hub saw it (a view's
/// memory, lost on restart: then the first sight counts).
pub type Since = BTreeMap<String, (Status, u64)>;

/// The snapshot's agents as rows. `user_kind`: the hub's (his cards; an
/// agent's `waits` counts its open ones); `vision`: the hub's model
/// catalog's rule (`bise_catalog::Catalog::vision`, K4), read at each
/// call so a /model change shows on the next agents event.
#[allow(clippy::too_many_arguments)]
pub fn agents(
    snap: &Snapshot,
    since: &mut Since,
    now: u64,
    user_kind: fn(&str) -> bool,
    vision: &dyn Fn(&str) -> Option<bool>,
    usage: &dyn Fn(&str) -> Option<bise_proto::rows::AgentUsage>,
    // its head (heads.rs): its newest entry's pos
    head: &dyn Fn(&str) -> Option<bise_proto::Pos>,
    which: Which,
) -> Vec<Agent> {
    let keep = |a: &&AgentRow| which == Which::All || a.status != crate::model::Status::Archived;
    let out: Vec<Agent> = snap.agents.iter().filter(keep).map(|a| agent(snap, a, since, now, user_kind, vision, usage, head)).collect();
    since.retain(|n, _| snap.agents.iter().any(|a| &a.name == n));
    out
}

/// Which agents [`agents`] makes rows of: every one (the older clients'
/// shape, agents-light's shim), or the live ones (the archived go as
/// [`archived`]'s light rows).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Which {
    All,
    Live,
}

/// agents-light (architect m_37000/m_37220): the archived agents as light
/// rows, the snapshot's order: their identity (name, aliases, dir),
/// status and since when as [`agent`] says them, and [`about`].
pub fn archived(snap: &Snapshot, since: &mut Since, now: u64) -> Vec<ArchivedAgent> {
    snap.agents
        .iter()
        .filter(|a| a.status == crate::model::Status::Archived)
        .map(|a| {
            let (status, _) = Status::of_hub(a.status.as_str());
            ArchivedAgent {
                name: a.name.clone(),
                aliases: a.aliases.clone(),
                dir: some(&a.dir).unwrap_or_else(|| a.name.clone()),
                status,
                since_ms: since_of(a, status, since, now),
                about: about(&title_of(a), &purpose_of(a)),
            }
        })
        .collect()
}

/// What an archived row says of an agent: its title, else its purpose
/// (the sidebar's rule; the hub's one choice).
pub fn about(title: &str, purpose: &str) -> String {
    if title.is_empty() { purpose } else { title }.to_string()
}

/// `agent/get`: one agent's full row (archived or not), by the same fn as
/// [`agents`]' rows.
#[allow(clippy::too_many_arguments)]
pub fn agent_named(
    snap: &Snapshot,
    name: &str,
    since: &mut Since,
    now: u64,
    user_kind: fn(&str) -> bool,
    vision: &dyn Fn(&str) -> Option<bool>,
    usage: &dyn Fn(&str) -> Option<bise_proto::rows::AgentUsage>,
    head: &dyn Fn(&str) -> Option<bise_proto::Pos>,
) -> Option<Agent> {
    let a = snap.agents.iter().find(|a| a.name == name)?;
    Some(agent(snap, a, since, now, user_kind, vision, usage, head))
}

/// Since when an agent has its status: kept while it keeps it, else now;
/// first seen: its report's time (not working), else its creation.
fn since_of(a: &AgentRow, status: Status, since: &mut Since, now: u64) -> u64 {
    let report_ms = a.report_ms.filter(|m| *m > 0);
    let created_ms = Some(a.created_ms).filter(|m| *m > 0);
    let first = report_ms.filter(|_| !status.working()).or(created_ms).unwrap_or(now);
    let at = match since.get(&a.name) {
        Some((st, at)) if *st == status => *at,
        Some(_) => now,
        None => first,
    };
    since.insert(a.name.clone(), (status, at));
    at
}

/// Its title: what it says it does now, else its role, its report, its
/// objective (one line).
fn title_of(a: &AgentRow) -> String {
    let title = [some(&a.note), some(&a.role), a.report.as_deref().and_then(some), some(&a.objective)].into_iter().flatten().next().unwrap_or_default();
    one_line(&title, 120)
}

/// Its objective's first line.
fn purpose_of(a: &AgentRow) -> String {
    one_line(a.objective.trim(), 200)
}

/// The agents whose head and usage the hub seeds from their buffered
/// lines: the live ones only (agents-light (a), architect m_37151). An
/// archived row shows no head nor gauge, and folding his 888 agents' tails
/// was 90 % of `initialize`; a restored agent is live again, so the next
/// agents event seeds it.
pub fn seeds(snap: &Snapshot) -> impl Iterator<Item = &str> {
    snap.agents.iter().filter(|a| a.status != crate::model::Status::Archived).map(|a| a.name.as_str())
}

/// A non-empty, trimmed text.
fn some(s: &str) -> Option<String> {
    Some(s.trim().to_string()).filter(|s| !s.is_empty())
}

#[allow(clippy::too_many_arguments)]
fn agent(
    snap: &Snapshot,
    a: &AgentRow,
    since: &mut Since,
    now: u64,
    user_kind: fn(&str) -> bool,
    vision: &dyn Fn(&str) -> Option<bool>,
    usage: &dyn Fn(&str) -> Option<bise_proto::rows::AgentUsage>,
    head: &dyn Fn(&str) -> Option<bise_proto::Pos>,
) -> Agent {
    let name = a.name.clone();
    let (status, archived) = Status::of_hub(a.status.as_str());
    let working = status.working();
    let report_ms = a.report_ms.filter(|m| *m > 0);
    let created_ms = Some(a.created_ms).filter(|m| *m > 0);
    let at = since_of(a, status, since, now);
    let report_text = a.report.as_deref().and_then(some);
    let worktree = a.mode == crate::model::Mode::Worktree;
    let report = report_text.map(|text| Report {
        kind: match status {
            Status::Done => ReportKind::Done,
            Status::Failed => ReportKind::Failed,
            Status::Blocked => ReportKind::Blocked,
            _ => ReportKind::Progress,
        },
        text,
        at_ms: report_ms.unwrap_or(0),
    });
    let waits = snap.cards.iter().filter(|c| c.agent.trim() == name && his(c.kind.trim(), user_kind)).count() as u32;
    let branch = a.branch.as_deref().and_then(some);
    let model = a.model.as_ref().and_then(|m| some(&m.name));
    Agent {
        main: a.main,
        status,
        phase: Phase::of_hub(a.status.as_str()),
        archived,
        title: title_of(a),
        purpose: purpose_of(a),
        since_ms: at,
        waits,
        parent: a.parent.as_deref().and_then(some),
        // its worktree's branch as released; the shared checkout's
        // branch apart (P4b-fix, the wire rule)
        branch: branch.clone().filter(|_| worktree),
        checkout_branch: branch.filter(|_| !worktree),
        worktree: some(&a.path).filter(|_| worktree),
        turn_ms: a.turn_ms.filter(|_| working),
        report,
        // his queued inputs (sb-core holds them, amb-hub 7dcb56ab)
        queued: a.queued_inputs.clone(),
        // where its thread is on disk, for stream C's readers (S2 step 3)
        dir: some(&a.dir).unwrap_or_else(|| name.clone()),
        aliases: a.aliases.clone(),
        // K4: its model and whether it reads images (the composer
        // refuses an image when false)
        vision: model.as_deref().and_then(vision),
        model,
        effort: a.model.as_ref().and_then(|m| some(&m.effort)),
        // S13: its context after its last call (the hub's live usage)
        usage: usage(&name),
        // R9/S3: model.rs's waiting_on, the field the TUI's panel reads
        waiting_on: bise_proto::rows::WaitingOn::of_word(a.waiting_on.as_deref().unwrap_or("").trim()),
        // event-wake: what it waits for (wake.rs `Wakes::watching`)
        watching: a.watching.clone(),
        // les bims: what it is doing now (snapshot_rows, crate::doing::row)
        doing: a.doing.clone(),
        // P4b: the terminal's facts, each the snapshot's own field
        mode: Some(if worktree { AgentMode::Worktree } else { AgentMode::Shared }),
        path: a.path.trim().to_string(),
        objective: a.objective.trim().to_string(),
        note: a.note.trim().to_string(),
        role: a.role.trim().to_string(),
        msgs_queued: a.queued as u32,
        inbox: a.inbox as u32,
        created_ms,
        place: a.place.as_deref().and_then(some),
        place_id: a.place_id.trim().to_string(),
        efforts: a.model.as_ref().map(|m| m.efforts.clone()).unwrap_or_default(),
        changes: a.model.as_ref().and_then(|m| m.changes),
        last_pos: head(&name),
        // issue 22: its ended turns from the same sb-core view as its
        // status (set_rt's count), never from its lines
        turns: a.turns,
        // running processes: Hub.procs, daemon/processes.rs's counts
        procs: a.procs.0,
        procs_port: a.procs.1,
        // agent-rank: set over the whole list (rank::apply), never per row
        rank: None,
        name,
    }
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
pub fn places(snap: &Snapshot) -> Vec<bise_proto::rows::Place> {
    snap.places
        .iter()
        .cloned()
        .map(|p| bise_proto::rows::Place {
            pr: p.pr.as_ref().map(|pr| crate::forge::news::pr_row(pr, &p.agents)),
            id: p.id,
            branch: p.branch,
            agents: p.agents,
            lid: p.lid,
            landing: p.landing,
            feature: p.feature,
            trying: p.trying,
            path: p.path,
        })
        .collect()
}

/// S10: the followed agents of a hub snapshot, each a running job: its
/// last progress (else its objective), `--step n/m`, since its last
/// report (else its creation). A follow ends with its job (job_end), so
/// every row runs.
pub fn jobs(snap: &Snapshot) -> Vec<Followed> {
    snap.agents
        .iter()
        .filter(|a| a.follow)
        .map(|a| {
            let progress = a.report.as_deref().and_then(some).filter(|_| a.report_kind.as_deref().map(str::trim) == Some("progress"));
            Followed {
                agent: a.name.trim().to_string(),
                label: one_line(&progress.unwrap_or_else(|| a.objective.trim().to_string()), 120),
                step: a.step,
                of: a.of,
                state: JobState::Running,
                since_ms: a.report_ms.filter(|m| *m > 0).or(Some(a.created_ms).filter(|m| *m > 0)).unwrap_or(0),
            }
        })
        .collect()
}

/// His cards: the user's kinds, a `drop` too (drop-card, architect
/// m_35666: he answers it with its 1/2 options, so the banner posts it).
fn his(kind: &str, user_kind: fn(&str) -> bool) -> bool {
    user_kind(kind)
}

/// The snapshot's open cards as rows, in its order: his (`HubEv::Cards`'
/// `cards`), then the others (`others`, P4c-4a: bise's own, a drop, a
/// done, a blocked..., which the terminal's inbox draws too). Each with
/// the snapshot's words (`text`, its `question` and `options` parsed
/// from them), note, place, PR, link and the message it answers.
pub fn cards(snap: &Snapshot, project: &str, now: u64, user_kind: fn(&str) -> bool) -> (Vec<Card>, Vec<Card>) {
    snap.cards
        .iter()
        .map(|c| {
            let kind = c.kind.trim().to_string();
            let (q, mut options) = question(&c.text);
            let page = c.page.as_ref().map(|p| CardPage {
                id: p.id.trim().to_string(),
                url: p.url.trim().to_string(),
                block: p.block.as_deref().and_then(some),
                item: p.item.as_deref().and_then(some),
                title: p.title.as_deref().and_then(some),
                v: p.v,
            });
            // a page's pick: each option's why, the agent's own (snapshot_pages)
            if let Some(p) = &c.pick {
                for o in &mut options {
                    let i = o.n as usize;
                    o.why = p.options.get(i.wrapping_sub(1)).map(|x| x.why.clone()).filter(|w| !w.is_empty());
                    o.mine = p.mine == Some(i);
                }
            }
            Card {
                id: c.id,
                project: project.to_string(),
                urgent: kind == "confirm",
                approval: Card::approval_kind(&kind),
                rank: Some(bise_proto::rows::card_rank(&kind)),
                agent: c.agent.trim().to_string(),
                question: q,
                options,
                since_ms: now.saturating_sub(c.age_ms),
                page,
                // what it is about, any artifact (card-links)
                artifact: c.artifact.clone(),
                // V14: the signin card's stopped agents (core.rs signin_waiting)
                waiting: c.waiting.clone(),
                note: c.note.as_deref().and_then(some),
                place: c.place.as_deref().and_then(some),
                pr: c.pr,
                link: c.link.as_deref().and_then(some),
                release: c.release.clone(),
                for_msg: c.for_msg,
                // a drafts batch's fields (daemon/page_cards.rs snapshot_card_links)
                batch: c.batch.clone(),
                // his answer runs (a merge, a try, an update): still answered
                running: c.running,
                text: c.text.clone(),
                kind,
            }
            // card-state (architect m_23450): each option says what a client does itself;
            // an option its asker set to open an artifact (notif-open) says that
            .with_acts(&c.opens)
        })
        .partition(|c| his(&c.kind, user_kind))
}

/// The art store's `artifacts` event (daemon/art.rs `artifacts_ev`:
/// rows + `seen_ms`) as rows. `url`: a page's the page server gives
/// (`page_url`), else its link or its file (its kept copy first); `new`:
/// not his own, changed since he last looked; the rest of each row as
/// the store gave it (the terminal's /artifacts reads it all).
/// `frame_base`: the page server's base (`http://127.0.0.1:<port>`, None
/// with no server): a copied file's `frame_url` is it plus where the page
/// server frames it (pages::site::frame_href: what bise's viewer draws, an
/// office file, a site, an image or a pdf, at `/a/<id>/view`; any other
/// file at its own link); a link or a page gets none.
pub fn artifacts(ev: &Value, page_url: impl Fn(&str) -> Option<String>, frame_base: Option<&str>) -> Vec<Artifact> {
    let seen = ev.get("seen_ms").and_then(Value::as_u64).unwrap_or(0);
    let rows = ev.get("rows").and_then(Value::as_array).cloned().unwrap_or_default();
    rows.iter()
        .map(|r| {
            let (id, kind) = (s(r, "id"), s(r, "kind"));
            let at_ms = r.get("ts_ms").and_then(Value::as_u64).unwrap_or(0);
            let url = match page_url(&id).filter(|_| s(r, "by") == "page") {
                Some(u) => u,
                // a local site bise serves again: its door (artifacts_serve)
                None => Some(s(r, "live"))
                    .filter(|l| !l.is_empty())
                    .or_else(|| Some(s(r, "copy")).filter(|c| !c.is_empty() && !s(r, "target").contains("://")))
                    .unwrap_or_else(|| s(r, "target")),
            };
            let target = s(r, "target");
            let path = Some(target.clone()).filter(|t| s(r, "by") != "page" && !crate::artifacts::is_link(t) && Path::new(t).is_absolute());
            let version = r.get("v").and_then(Value::as_u64).unwrap_or(0);
            let frame_url = frame_base
                .filter(|_| crate::pages::site::serves_copy(&s(r, "by"), &s(r, "copy")))
                .map(|b| format!("{b}{}", crate::pages::site::frame_href(&id, version, &s(r, "copy"), true)));
            let versions = r.get("versions").and_then(Value::as_array).map_or_else(Vec::new, |vs| {
                vs.iter()
                    .map(|x| {
                        let n = x.get("v").and_then(Value::as_u64).unwrap_or(0);
                        ArtifactVersion {
                            v: n as u32,
                            at_ms: x.get("ts_ms").and_then(Value::as_u64).unwrap_or(0),
                            note: Some(s(x, "note")).filter(|n| !n.is_empty()),
                            target: s(x, "target"),
                            copy: Some(s(x, "copy")).filter(|c| !c.is_empty()),
                            // the row's rule, per version (architect m_23970)
                            frame_url: frame_base
                                .filter(|_| crate::pages::site::serves_copy(&s(r, "by"), &s(x, "copy")))
                                .map(|b| format!("{b}{}", crate::pages::site::frame_href(&id, n, &s(x, "copy"), n == version))),
                        }
                    })
                    .collect()
            });
            let pr = r.get("pr").filter(|p| p.is_object()).map(|p| ArtifactPr { repo: s(p, "repo"), number: p.get("number").and_then(Value::as_u64).unwrap_or(0) });
            Artifact {
                path,
                frame_url,
                versions,
                title: s(r, "title"),
                agent: s(r, "agent"),
                version: version as u32,
                new: s(r, "by") != "you" && at_ms > seen,
                at_ms,
                url,
                kind,
                id,
                by: s(r, "by"),
                archived: r.get("archived").and_then(Value::as_bool).unwrap_or(false),
                archived_at: r.get("archived_at").and_then(Value::as_u64),
                created_ms: r.get("created_ms").and_then(Value::as_u64),
                copy: Some(s(r, "copy")).filter(|c| !c.is_empty()),
                gone: r.get("gone").and_then(Value::as_bool).unwrap_or(false),
                detail: s(r, "detail"),
                pr,
                keys: r.get("keys").and_then(Value::as_array).map_or_else(Vec::new, |k| k.iter().filter_map(|x| x.as_str().map(String::from)).collect()),
                // daemon/art.rs art_live_rows: bise_proto::office::live_words
                live: r.get("live").and_then(|l| serde_json::from_value(l.clone()).ok()),
                folder: Some(s(r, "folder")).filter(|f| !f.is_empty()),
                turn: r.get("turn").and_then(Value::as_u64),
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
        cut_files: ev.get("cut_files").and_then(Value::as_u64).filter(|n| *n > 0),
        view: Box::new(DiffView {
            title: Some(s(ev, "title")).filter(|t| !t.is_empty()),
            req: ev.get("req").and_then(Value::as_u64),
            commits: ev.get("commits").and_then(Value::as_u64),
            working: flag(ev, "working"),
            uncommitted: flag(ev, "uncommitted"),
            landed_ms: ev.get("landed_ms").and_then(Value::as_u64),
            gone: flag(ev, "gone"),
            artifact: Some(s(ev, "artifact")).filter(|a| !a.is_empty()),
            version: ev.get("version").and_then(Value::as_u64).map(|v| v as u32),
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
    use bise_proto::thread::lines::{read, Obs, Rec};
    matches!(read(line), Rec::Obs(Obs::TurnDone(_))).then(String::new)
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

    /// The lines of the newest KEEP entries stay, and every line of a call
    /// one of them has (its result and finish come after a card line
    /// written while it runs: `thread::tail_start`).
    fn trim(&mut self, entries: &[Entry]) {
        if entries.len() > KEEP {
            let cut = thread::tail_start(&self.lines, entries, entries.len() - KEEP);
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
    use crate::card_link::PageLink;
    use crate::snapshot::CardRow;
    use serde_json::json;

    fn uk(k: &str) -> bool {
        matches!(k, "question" | "drop" | "confirm")
    }

    /// Law (proto-tests m_15071): a drafts-batch card's `batch` (the
    /// snapshot's, daemon/page_cards.rs) is hub/cards' `Card.batch` with
    /// every key and value; any other card has none.
    #[test]
    fn a_drafts_batch_card_carries_its_batch() {
        let batch = json!({"count": 3, "title": "inbox replies", "what": "replies", "names": ["legal", "Lucas", "Marc"],
            "topics": ["the contract", "the dates", "the invoice"], "line": "replies to legal, Lucas and Marc", "actions": "1 to close"});
        let card = |id: u64, text: &str| CardRow { id, kind: "question".into(), agent: "main".into(), text: text.into(), ..CardRow::default() };
        let page = PageLink { id: "replies".into(), block: Some("drafts".into()), item: Some("d1".into()), drafts: true, url: "http://p/replies#d1".into(), ..PageLink::default() };
        let snap = Snapshot {
            cards: vec![
                CardRow { page: Some(page), batch: serde_json::from_value(batch.clone()).ok(), ..card(7, "send the 3 replies?\n1. send\n2. open the page") },
                card(8, "ok?"),
            ],
            ..Snapshot::default()
        };
        let (his, _) = cards(&snap, "p", 10, uk);
        let typed = serde_json::to_value(his[0].batch.as_ref().expect("its batch")).unwrap();
        assert_eq!(typed, batch, "every key of the drafts' info, the same value");
        assert_eq!(his[1].batch, None);
    }

    /// Law (card-state, architect m_23450): each option of a card row says
    /// what a client does itself, by bise-proto's one rule (`opt_act`, the
    /// TUI's pick() calls it too): a merge's 2 opens its PR (the link line
    /// of its text), an update's 3 its release page, a sign-in's 1 is the
    /// client's sign-in; every other option answers (none). His #384 came
    /// back because the window sent a merge's 2 as its answer.
    #[test]
    fn each_option_says_what_a_client_does_itself() {
        use bise_proto::rows::{opt_act, OptAct};
        let card = |id: u64, kind: &str, agent: &str, text: &str| CardRow { id, kind: kind.into(), agent: agent.into(), text: text.into(), ..CardRow::default() };
        let snap = Snapshot {
            cards: vec![
                CardRow { pr: Some(384), place: Some("p1".into()), ..card(384, "merge", "pattern-error-dx", "#384 is ready to merge\nP2 errors\ngithub.com/gvergnaud/ts-pattern/pull/384\n\n1. squash and merge\n2. open it on GitHub\n3. not yet") },
                card(6, "signin", "main", "sign in again?\n\n1. sign in again\n2. later"),
                card(7, "question", "docs", "where?\n1. here\n2. there"),
            ],
            ..Snapshot::default()
        };
        let (his, _) = cards(&snap, "p", 10, |_| true);
        let acts = |id: u64| his.iter().find(|c| c.id == id).unwrap().options.iter().map(|o| o.act.clone()).collect::<Vec<_>>();
        let pr = Some(OptAct::link(Some("https://github.com/gvergnaud/ts-pattern/pull/384")));
        assert_eq!(acts(384), vec![None, pr, None]);
        assert_eq!(acts(6), vec![Some(OptAct::SignIn), None]);
        assert_eq!(acts(7), vec![None, None]);
        // the row's act is the rule's for every kind and option (the TUI asks the same fn)
        for c in &his {
            let link = c.act_link();
            for o in &c.options {
                assert_eq!(o.act, opt_act(&c.kind, o.n, link.as_deref()), "{} {}", c.kind, o.n);
            }
        }
        // a merge whose text has no link: still the app's own, nothing to open
        let snap = Snapshot { cards: vec![CardRow { pr: Some(9), ..card(9, "merge", "x", "#9 is ready\n\n1. merge\n2. open it on GitHub\n3. not yet") }], ..Snapshot::default() };
        let (his, _) = cards(&snap, "p", 10, |_| true);
        assert_eq!(his[0].options[1].act, Some(OptAct::link(None)));
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

    fn snap() -> Snapshot {
        let card = |id: u64, kind: &str, agent: &str, text: &str, age_ms: u64| CardRow { id, kind: kind.into(), agent: agent.into(), text: text.into(), age_ms, ..CardRow::default() };
        Snapshot {
            agents: vec![
                AgentRow { main: true, created_ms: 5, ..AgentRow::test("main") },
                AgentRow {
                    status: crate::model::Status::Working,
                    objective: "make the e2e fast\nmore".into(),
                    role: "profiling the hub".into(),
                    mode: crate::model::Mode::Worktree,
                    branch: Some("sb/perf".into()),
                    path: "/w/perf".into(),
                    parent: Some("main".into()),
                    turn_ms: Some(4200),
                    created_ms: 7,
                    queued: 1,
                    queued_inputs: vec![bise_proto::draft::Queued { id: 3, text: "then the cold run".into(), created_ms: 8, parts: vec![] }],
                    ..AgentRow::test("perf")
                },
                AgentRow {
                    status: crate::model::Status::Archived,
                    objective: "an old fix".into(),
                    report: Some("fixed in 0.4".into()),
                    report_ms: Some(9),
                    branch: Some("main".into()),
                    path: "/w".into(),
                    ..AgentRow::test("old")
                },
            ],
            cards: vec![
                card(9, "question", "perf", "which bench?\n1. cold\n2. warm", 1000),
                CardRow { page: Some(PageLink { id: "p".into(), url: "http://x/p/p".into(), block: Some("q1".into()), ..PageLink::default() }), ..card(10, "confirm", "perf", "run it?", 0) },
                card(11, "drop", "old", "archive old?", 0),
                card(12, "done", "perf", "perf is done", 0),
            ],
            ..Snapshot::default()
        }
    }

    fn with_model(mut a: AgentRow, model: &str) -> AgentRow {
        a.model = Some(crate::snapshot::Model { name: model.into(), ..Default::default() });
        a
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
        s.agents[1] = with_model(s.agents[1].clone(), "mistral/codestral-latest");
        s.agents[2] = with_model(s.agents[2].clone(), "ollama/llava");
        let a = agents(&s, &mut since, 100, uk, &vision, &|_| None, &|_| None, Which::All);
        assert_eq!((a[1].model.as_deref(), a[1].vision), (Some("mistral/codestral-latest"), Some(false)), "listed, no vision");
        assert_eq!((a[2].model.as_deref(), a[2].vision), (Some("ollama/llava"), None), "unlisted: never a guess");
        assert_eq!((a[0].model.as_deref(), a[0].vision), (None, None), "no model in the snapshot");
        // his /model switch: the next rows follow
        s.agents[1] = with_model(s.agents[1].clone(), "anthropic/claude-haiku-4-5");
        assert_eq!(agents(&s, &mut since, 200, uk, &vision, &|_| None, &|_| None, Which::All)[1].vision, Some(true));
    }

    /// agents-light (a), architect m_37151: initialize never reads or
    /// folds an archived agent's lines; restored, it is seeded at the next
    /// agents event.
    #[test]
    fn only_live_agents_get_their_head_and_usage_seeded() {
        let mut s = snap();
        assert_eq!(seeds(&s).collect::<Vec<_>>(), ["main", "perf"], "the archived one is not seeded");
        s.agents[2].status = crate::model::Status::Idle;
        assert_eq!(seeds(&s).collect::<Vec<_>>(), ["main", "perf", "old"], "restored: seeded again");
    }

    /// agents-light (architect m_37220): the archived agents' light rows
    /// say what their full rows said (status, since, title as about,
    /// dir), and the live rows leave them out.
    #[test]
    fn an_archived_agent_is_a_light_row() {
        let mut since = Since::new();
        let full = agents(&snap(), &mut since, 100, uk, &|_| None, &|_| None, &|_| None, Which::All);
        let live = agents(&snap(), &mut since, 100, uk, &|_| None, &|_| None, &|_| None, Which::Live);
        assert_eq!(live.iter().map(|a| a.name.as_str()).collect::<Vec<_>>(), ["main", "perf"]);
        let light = archived(&snap(), &mut since, 100);
        let o = &full[2];
        assert_eq!(light.len(), 1);
        let l = &light[0];
        assert_eq!((l.name.as_str(), l.dir.as_str(), l.status, l.since_ms, l.about.as_str()), (o.name.as_str(), o.dir.as_str(), o.status, o.since_ms, o.title.as_str()));
        assert_eq!(about("", "make it fast"), "make it fast", "no title: its purpose");
        let mut s = snap();
        s.agents.truncate(2);
        assert!(archived(&s, &mut since, 100).is_empty());
        assert!(agent_named(&snap(), "old", &mut since, 100, uk, &|_| None, &|_| None, &|_| None).is_some_and(|a| a.archived && a.report.is_some()), "agent/get: the full row");
    }

    #[test]
    fn the_snapshot_becomes_rows() {
        let mut since = Since::new();
        let a = agents(&snap(), &mut since, 100, uk, &|_| None, &|_| None, &|_| None, Which::All);
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
        s2.agents[1].status = crate::model::Status::Idle;
        assert_eq!(agents(&s2, &mut since, 200, uk, &|_| None, &|_| None, &|_| None, Which::All)[1].since_ms, 200);
        assert_eq!(agents(&s2, &mut since, 300, uk, &|_| None, &|_| None, &|_| None, Which::All)[1].since_ms, 200);
        let (c, others) = cards(&snap(), "acme", 5_000, uk);
        // drop-card: a drop is his (he answers it), never in both lists
        assert_eq!(c.iter().map(|c| c.id).collect::<Vec<_>>(), [9, 10, 11], "his kinds, a drop too");
        // P4c-4a: bise's own (a done) are the others, in order
        assert_eq!(others.iter().map(|c| (c.id, c.kind.as_str())).collect::<Vec<_>>(), [(12, "done")]);
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
        let a = artifacts(&ev, |id| (id == "perf-notes").then(|| "http://127.0.0.1:1/p/perf-notes".to_string()), None);
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

    /// architect m_23970: each version's frame_url is the row's rule
    /// (pages::site::frame_href) for that version; art-live: the row's
    /// live words come through as the store's row has them.
    #[test]
    fn each_version_gets_its_frame_url_and_the_row_its_live_words() {
        let ev = json!({"ev": "artifacts", "seen_ms": 0, "rows": [
            {"id": "q3", "kind": "slides", "title": "q3.pptx", "agent": "deck", "by": "deck", "ts_ms": 90, "v": 2,
             "target": "/w/q3.pptx", "copy": "/st/artifacts/q3/v2/q3.pptx", "live": {"line": "v2 · 1 slide changed", "back": {"text": "← v1", "v": 1}}, "versions": [
                {"v": 1, "ts_ms": 80, "target": "/w/q3.pptx", "copy": "/st/artifacts/q3/v1/q3.pptx"},
                {"v": 2, "ts_ms": 90, "target": "/w/q3.pptx", "copy": "/st/artifacts/q3/v2/q3.pptx"},
                {"v": 3, "ts_ms": 95, "target": "/w/q3.pptx", "copy": null}]}]});
        let base = "http://127.0.0.1:47123";
        let a = artifacts(&ev, |_| None, Some(base));
        let urls: Vec<Option<String>> = a[0].versions.iter().map(|x| x.frame_url.clone()).collect();
        let href = |v: u64, newest: bool| Some(format!("{base}{}", crate::pages::site::frame_href("q3", v, "/st/artifacts/q3/v1/q3.pptx", newest)));
        assert_eq!(urls, [href(1, false), href(2, true), None], "older: /a/q3/1/view, newest: /a/q3/view, no copy: none");
        assert_eq!(a[0].frame_url, urls[1], "the row's and its newest version's are the same");
        assert_eq!(a[0].live.as_ref().and_then(|l| l.line.as_deref()), Some("v2 · 1 slide changed"));
        assert_eq!(a[0].live.as_ref().and_then(|l| l.back.as_ref()).map(|b| b.v), Some(1));
        assert!(artifacts(&ev, |_| None, None)[0].versions.iter().all(|x| x.frame_url.is_none()), "no page server: none");
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

    /// tui-parity R1 (proto-lead m_15643): a card line written while its
    /// `sb card` call runs is an entry; once it is the oldest one kept,
    /// the call's result and finish still fold into the call's row, never
    /// a lone `#2 ✓` entry a turn later. Every line pushed one by one,
    /// each change an entry the whole fold has.
    #[test]
    fn a_live_tail_never_cuts_a_call_from_its_finish() {
        let none = |_: &str| None;
        let open: [u64; 0] = [];
        let ctx = Ctx { open_cards: &open, page: &none, provider: &|i: &str, _: &str| i.to_string(), width: &unicode_width::UnicodeWidthStr::width, offset: &|_| 0, attached: &crate::attached::split };
        let mut lines = vec!["sb you : ask me", "  obs: turn_started", "  obs: tool_started #2", "tool #2 bash : sb card \"second?\"", "sb card : #2 question @main : second?"];
        let filler: Vec<String> = (0..KEEP + 2).map(|i| format!("sb info : note {i}")).collect();
        lines.extend(filler.iter().map(String::as_str));
        lines.extend(["tool_result #2 ok : #2", "  obs: tool_finished #2 ok", "  obs: turn_done: completed"]);
        lines.extend(["sb you : the user answered card #2", "  obs: turn_started", "  obs: assistant: ack", "  obs: turn_done: completed"]);
        let after: Vec<String> = (0..KEEP).map(|i| format!("sb info : later {i}")).collect();
        lines.extend(after.iter().map(String::as_str));
        let all: Vec<Line> = lines.iter().enumerate().map(|(i, l)| (i as u64 + 1, 1_000 + i as u64, l.to_string())).collect();
        let whole = thread::fold(&all, &ctx);
        let mut live = Live::start(Vec::new(), &[]);
        for l in &all {
            for e in live.push(l.clone(), &ctx) {
                assert!(whole.iter().any(|w| w.pos == e.pos), "an entry the whole fold hasn't: {e:?}");
            }
        }
        let row = whole.iter().find_map(|e| e.tools.as_ref()).expect("the call's row");
        assert_eq!((row.count, row.items[0].state), (1, bise_proto::thread::ToolState::Ok));
        assert_eq!(whole.iter().filter(|e| e.kind == thread::EntryKind::Tools).count(), 1);
        // KEEP entries past its finish, the call's lines go
        assert!(live.lines.first().is_some_and(|l| l.0 > 3), "{:?}", live.lines.first());
    }
}
