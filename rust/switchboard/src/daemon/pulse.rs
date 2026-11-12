//! The project's pulse on the hub's side (the H1 projects sidebar,
//! architect m_44800 and m_44825; the pure part is `crate::pulse`, the
//! shape bise_proto's `rows::Pulse`). Runtime view state: it observes,
//! decides nothing, wakes nobody; a restart reads it all again.
//!
//! - The week comes from the one transcript reader, `daemon/usage.rs`'s
//!   fill (started at the first tick, on its thread): each agent's turns
//!   and main's entries written for him. From the fill's start on, the
//!   feed's lines come here as the transcript gets them ([`Shell::pulse_line`]:
//!   a turn line, or main's line kept for its fold); the fill counts the
//!   lines written before it started, so none counts twice. Until it is
//!   back the pulse says `counting`.
//! - The news candidates: a failed agent (the end of its last turn when
//!   that turn failed, a fact on disk; else when this hub saw it fail),
//!   the newest land on the trunk (`lands.jsonl`, only its new bytes read
//!   each time), the artifacts (cached at each `artifacts` push), the
//!   timers' next runs. `crate::pulse::news` picks one.
//! - Sent: `hub/pulse` in initialize's state, then from the tick at most
//!   once a minute and only when it changed ([`Shell::pulse_check`]).
//!   `view.json` carries the last value (daemon/projects.rs), a fresh one
//!   at a clean stop. Never to agent.sock.

use super::*;
use crate::pulse::{self, Said, Turns};
use bise_proto::rows::{News, NewsWhat, Pulse};
use bise_proto::thread::Ctx;

/// What the week's read gave, back from the fill's thread.
#[derive(Debug, Default)]
pub(super) struct Week {
    /// by agent folder
    pub turns: BTreeMap<String, Turns>,
    pub said: Said,
}

/// The pulse's state (one field of the Shell).
#[derive(Default)]
pub(super) struct PulseState {
    /// by agent folder: the live side's turns until the fill is back,
    /// then the fill's and the live side's joined
    turns: BTreeMap<String, Turns>,
    /// the fill is back
    filled: bool,
    /// main's entries for him (the fill's state, then the live lines)
    said: Said,
    /// main's live lines while the fill reads
    said_live: Vec<(u64, String)>,
    /// agents this hub saw turn failed, and when
    failed_seen: BTreeMap<String, u64>,
    /// agents this hub saw not failed
    seen_ok: BTreeSet<String>,
    /// lands.jsonl: how far it was read, and the newest land on the trunk
    lands_read: u64,
    landed: Option<News>,
    /// the artifacts (at_ms, id, title), from the last `artifacts` push
    arts: Option<Vec<(u64, String, String)>>,
    sent: Option<Pulse>,
    value: Option<Pulse>,
}

/// A fold context for main's lines: only the entries' kinds and times
/// are read, so no cards, pages or providers.
pub(super) fn with_ctx<T>(f: impl FnOnce(&Ctx) -> T) -> T {
    let none = |_: &str| None;
    let ctx = Ctx { open_cards: &[], page: &none, provider: &|i: &str, _: &str| i.to_string(), width: &|s: &str| s.chars().count(), offset: &|_| 0, attached: &crate::attached::split };
    f(&ctx)
}

impl Shell {
    /// A line the feed wrote in `name`'s transcript at `ts`. Before the
    /// fill started: nothing (the fill will read it).
    pub(super) fn pulse_line(&mut self, name: &str, ts: u64, line: &str) {
        if !self.usage_started() {
            return;
        }
        let Some(dir) = self.dir_of(name) else { return };
        let l = pulse::turn_line(line);
        self.pulse.turns.entry(dir).or_default().line(ts, l);
        if name == crate::model::MAIN {
            match self.pulse.filled {
                true => self.pulse.said.push(ts, line.to_string()),
                false => self.pulse.said_live.push((ts, line.to_string())),
            }
        }
    }

    /// The fill is back: its turns joined with the live side's, main's
    /// fold continued with the lines that came meanwhile.
    pub(super) fn pulse_filled(&mut self, week: Week) {
        let busy: BTreeSet<String> = self.hub.st.agents.values().filter(|a| a.turn_ms.is_some()).map(|a| a.dir.clone()).collect();
        let mut live = std::mem::take(&mut self.pulse.turns);
        for (dir, fill) in week.turns {
            let l = live.remove(&dir).unwrap_or_default();
            let b = busy.contains(&dir);
            self.pulse.turns.insert(dir, Turns::merge(fill, l, b));
        }
        self.pulse.turns.extend(live);
        let mut said = week.said;
        for (ms, l) in std::mem::take(&mut self.pulse.said_live) {
            said.push(ms, l);
        }
        with_ctx(|ctx| said.settle(ctx));
        self.pulse.said = said;
        self.pulse.filled = true;
        // the next tick sends it
        self.tick.again(super::tick::PULSE);
    }

    /// The `artifacts` push's rows (daemon/proto/emit.rs): kept for the
    /// pulse's times and news.
    pub(super) fn pulse_artifacts(&mut self, items: &[bise_proto::rows::Artifact]) {
        self.pulse.arts = Some(items.iter().map(|a| (a.at_ms, a.id.clone(), a.title.clone())).collect());
    }

    /// On the tick, at most once a minute (daemon/tick.rs): the fill
    /// starts at the first one; the pulse to every typed connection when
    /// it changed.
    pub(super) fn pulse_check(&mut self) {
        self.week_fill_start();
        let now = now_ms();
        let p = self.pulse_now(now);
        if self.pulse.sent.as_ref() == Some(&p) {
            return;
        }
        self.pulse.sent = Some(p.clone());
        let ev = HubEv::Pulse { project: self.project(), pulse: p };
        let ids = self.typed_ids();
        self.proto_note(&ids, &ev);
    }

    /// `hub/pulse` now (initialize's state; not marked sent).
    pub(super) fn pulse_ev(&mut self) -> HubEv {
        let p = match self.pulse.value.clone() {
            Some(p) => p,
            None => self.pulse_now(now_ms()),
        };
        HubEv::Pulse { project: self.project(), pulse: p }
    }

    /// view.json's value: the last computed (`fresh`: computed now, a stop).
    pub(super) fn pulse_view(&mut self, fresh: bool) -> Pulse {
        match self.pulse.value.clone() {
            Some(p) if !fresh => p,
            _ => self.pulse_now(now_ms()),
        }
    }

    /// The pulse at `now` (kept as the last value).
    fn pulse_now(&mut self, now: u64) -> Pulse {
        let from = pulse::window_from(now, &bise_home::clock::offset_at);
        for t in self.pulse.turns.values_mut() {
            t.forget_before(from);
        }
        let busy = pulse::buckets(self.pulse.turns.values().flat_map(|t| t.intervals(now)), from);
        let last_ms = self.pulse.turns.values().filter_map(|t| t.last_ms(now)).max();
        let said_at = match self.pulse.filled {
            true => {
                let said = &mut self.pulse.said;
                with_ctx(|ctx| said.settle(ctx));
                said.forget_before(from);
                said.times(from)
            }
            // counting: what the live lines gave
            false => {
                let mut s = Said::default();
                with_ctx(|ctx| s.push_all(self.pulse.said_live.iter().cloned(), ctx));
                s.times(from)
            }
        };
        if self.pulse.arts.is_none() {
            // none before the artifact rows' first build (daemon/art_rows.rs)
            if let Some(rows) = self.artifact_rows_now() {
                self.pulse_artifacts(&rows);
            }
        }
        let arts = self.pulse.arts.as_deref().unwrap_or_default();
        let artifacts_at = pulse::cap(arts.iter().map(|a| a.0).filter(|t| *t >= from).collect());
        let cands = self.news_candidates(now);
        let p = Pulse { from_ms: from, busy_ms: busy, last_ms, news: pulse::news(&cands, now), artifacts_at, said_at, counting: !self.pulse.filled };
        self.pulse.value = Some(p.clone());
        p
    }

    /// Every news candidate now (pulse::news picks).
    fn news_candidates(&mut self, now: u64) -> Vec<News> {
        let mut out = Vec::new();
        // failed: the end of its last turn when that turn failed, else
        // when this hub saw it fail (no news when it never did)
        let agents: Vec<(String, String, bool)> = self.hub.st.agents.values().filter(|a| a.lifecycle != crate::model::Lifecycle::Archived).map(|a| (a.name.clone(), a.dir.clone(), a.status() == crate::model::Status::Failed)).collect();
        for (name, dir, failed) in agents {
            if !failed {
                self.pulse.seen_ok.insert(name.clone());
                self.pulse.failed_seen.remove(&name);
                continue;
            }
            if self.pulse.seen_ok.remove(&name) {
                self.pulse.failed_seen.insert(name.clone(), now);
            }
            let at = match self.pulse.turns.get(&dir).and_then(|t| t.last_end) {
                Some((ms, true)) => Some(ms),
                _ => self.pulse.failed_seen.get(&name).copied(),
            };
            if let Some(at_ms) = at {
                out.push(News { at_ms, what: NewsWhat::Failed { agent: name } });
            }
        }
        self.pulse_lands();
        out.extend(self.pulse.landed.clone());
        if let Some((at_ms, id, title)) = self.pulse.arts.as_ref().and_then(|a| a.iter().max_by_key(|a| a.0)) {
            out.push(News { at_ms: *at_ms, what: NewsWhat::Artifact { id: id.clone(), title: title.clone() } });
        }
        for t in crate::proto_view::scheduled(self.hub.timers()) {
            if let Some(at_ms) = t.next_ms {
                out.push(News { at_ms, what: NewsWhat::Scheduled { id: t.id.to_string(), name: t.name } });
            }
        }
        out
    }

    /// lands.jsonl's new lines since the last read: the newest land on
    /// the trunk (its short name as the last features refresh read it).
    fn pulse_lands(&mut self) {
        use std::io::{Read, Seek, SeekFrom};
        let path = self.opts.paths.lands();
        let Ok(mut f) = std::fs::File::open(&path) else { return };
        let len = f.metadata().map(|m| m.len()).unwrap_or(0);
        if len < self.pulse.lands_read {
            // a new file: read it whole
            self.pulse.lands_read = 0;
        }
        if len == self.pulse.lands_read || f.seek(SeekFrom::Start(self.pulse.lands_read)).is_err() {
            return;
        }
        let mut text = String::new();
        if f.read_to_string(&mut text).is_err() {
            return;
        }
        // whole lines only: a line being written is read next time
        let Some(end) = text.rfind('\n') else { return };
        self.pulse.lands_read += end as u64 + 1;
        let trunk = self.features.trunk();
        let trunk = if trunk.is_empty() { "main".to_string() } else { trunk };
        for l in text[..end].lines() {
            let Ok(r) = serde_json::from_str::<crate::land_record::LandRecord>(l) else { continue };
            if crate::land::short(&r.target) != trunk {
                continue;
            }
            let sha: String = r.tip.chars().take(9).collect();
            self.pulse.landed = Some(News { at_ms: r.at_ms, what: NewsWhat::Landed { agent: r.agent, sha, target: trunk.clone() } });
        }
    }
}
