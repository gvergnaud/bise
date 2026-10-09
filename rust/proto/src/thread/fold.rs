//! The fold: an agent's transcript lines, read by [`super::lines`], as
//! entries (consecutive tool calls are one `tools` entry at the first
//! call's pos; a message on several lines is one entry).

use super::lines::{self, Delivered, Hub, Mark, Obs, Rec, ToolMove};
use super::scheduled;
use super::words::{self, one_line, summary};
use super::{Answered, ApprovalFold, Ctx, Entry, EntryCard, EntryKind, Landed, Line, Made, NotDelivered, Notice, PageRef, PrNews, ReportRef, Scheduled, Thinking, ToolItem, ToolKind, Tools};
use super::{cap, Delivery, FileCount, ToolState, TurnFailed};
use crate::context::FnContext;
use crate::rows::question;

struct Fold<'a> {
    out: Vec<Entry>,
    /// the entry a raw line continues (a message on several lines)
    cont: Option<usize>,
    /// tool id -> (entry, item)
    tools: Vec<(u32, usize, usize)>,
    /// the 'you' entry the previous line pushed: the only one a context
    /// line may attach to
    you: Option<usize>,
    /// the previous line's time (0: a replay), for a thinking's duration
    last_ms: u64,
    /// the first entry of the current turn (its `turn_started`'s marks)
    turn: usize,
    ctx: &'a Ctx<'a>,
}

impl Fold<'_> {
    fn push(&mut self, e: Entry) -> usize {
        self.out.push(e);
        self.out.len() - 1
    }

    /// One line's tool move (G3, [`lines::tool_move`]: the TUI's rule, a
    /// row exists from `tool_started`).
    fn tool_move(&mut self, pos: u64, ms: u64, m: ToolMove) {
        match m {
            ToolMove::Start(id) => self.start(pos, ms, id, ToolState::Run),
            ToolMove::Info { id, name, args } => self.info(pos, ms, id, &name, &args),
            ToolMove::Intent { id, text } => self.intent(id, &text),
            ToolMove::Code { id, code } => self.code(id, &code),
            ToolMove::Result { id, ok, preview } => self.result(id, ok, &preview, ms),
            ToolMove::Finish { id, ok } => self.finish(pos, ms, id, ok),
        }
    }

    /// `tool_started`: a running item, in the tools entry it continues
    /// (or a new one at this line).
    fn start(&mut self, pos: u64, ms: u64, id: u32, state: ToolState) {
        let e = match self.out.last() {
            Some(e) if e.kind == EntryKind::Tools => self.out.len() - 1,
            _ => {
                let mut e = Entry::new(pos, ms, EntryKind::Tools, String::new());
                e.tools = Some(Tools { count: 0, summary: String::new(), items: Vec::new() });
                self.push(e)
            }
        };
        let item = ToolItem {
            pos,
            id: u64::from(id),
            at_ms: ms,
            name: String::new(),
            args: String::new(),
            intent: None,
            text: String::new(),
            kind: ToolKind::Other,
            land: false,
            state,
            ms: None,
            exit: None,
            err: None,
            code: None,
            out: None,
            files: Vec::new(),
        };
        let items = &mut self.out[e].tools.as_mut().expect("a tools entry").items;
        items.push(item);
        let i = items.len() - 1;
        self.tools.push((id, e, i));
        self.sum(e);
    }

    /// `tool #<id> <name> : <args>`: the call's name and args on its
    /// item; a bash `sb report` or `sb page publish` is a report or a page
    /// entry instead (its item leaves).
    fn info(&mut self, pos: u64, ms: u64, id: u32, name: &str, args: &str) {
        let args = lines::unescape(args);
        if name == "bash" {
            if let Some((kind, text)) = lines::report_in(&args) {
                self.unstart(id);
                let mut e = Entry::new(pos, ms, EntryKind::Report, text);
                e.report = Some(ReportRef { kind });
                self.push(e);
                return;
            }
            if let Some(id_) = lines::publish_in(&args) {
                self.unstart(id);
                let page = (self.ctx.page)(&id_).unwrap_or(PageRef { id: id_.clone(), title: id_.replace('-', " "), v: None, url: String::new() });
                let v = page.v.map(|v| format!(" v{v}")).unwrap_or_default();
                let mut e = Entry::new(pos, ms, EntryKind::Page, format!("{}{v}", page.title));
                e.page = Some(page);
                self.push(e);
                return;
            }
        }
        let Some(&(_, e, _)) = self.tools.iter().rev().find(|(t, _, _)| *t == id) else { return };
        let Some(item) = self.item(id) else { return };
        item.text = item.intent.clone().unwrap_or_else(|| format!("{name}: {}", one_line(&args.replace("\\N", "\n"), 100)));
        item.kind = ToolKind::of(name);
        item.land = name == "bash" && args.contains("sb land");
        item.name = name.to_string();
        item.args = cap(&args);
        self.sum(e);
    }

    /// The item of call `id` leaves (a report's or a page's call): its
    /// tools entry too when it was its only one.
    fn unstart(&mut self, id: u32) {
        let Some(k) = self.tools.iter().rposition(|(t, _, _)| *t == id) else { return };
        let (_, e, i) = self.tools.remove(k);
        let Some(t) = self.out[e].tools.as_mut() else { return };
        t.items.remove(i);
        for r in self.tools.iter_mut().filter(|r| r.1 == e && r.2 > i) {
            r.2 -= 1;
        }
        if !t.items.is_empty() {
            self.sum(e);
            return;
        }
        self.out.remove(e);
        self.tools.retain(|r| r.1 != e);
        let shift = |x: usize| if x > e { x - 1 } else { x };
        for r in self.tools.iter_mut() {
            r.1 = shift(r.1);
        }
        self.cont = self.cont.filter(|&c| c != e).map(shift);
        self.you = self.you.filter(|&c| c != e).map(shift);
        self.turn = shift(self.turn);
    }

    /// `tool_finished`: the running item of call `id` ends, its duration
    /// from its start ([`words::tool_ms`]); none running: an ended item
    /// of its own (the TUI's row).
    fn finish(&mut self, pos: u64, ms: u64, id: u32, ok: bool) {
        let state = if ok { ToolState::Ok } else { ToolState::Err };
        let running = self.tools.iter().rev().find(|(t, _, _)| *t == id).map(|&(_, e, i)| (e, i));
        match running.and_then(|(e, i)| self.out[e].tools.as_mut().map(|t| &mut t.items[i])) {
            Some(item) if item.state == ToolState::Run => {
                item.state = state;
                item.ms = words::tool_ms(item.at_ms, ms);
            }
            _ => self.start(pos, ms, id, state),
        }
    }

    /// The call's item, by its tool id (the last one with that id).
    fn item(&mut self, id: u32) -> Option<&mut ToolItem> {
        let &(_, e, i) = self.tools.iter().rev().find(|(t, _, _)| *t == id)?;
        self.out[e].tools.as_mut().map(|t| &mut t.items[i])
    }

    /// `tool_code`: the call's full command or args, capped; an edit's
    /// files with their line counts (its code is the patch).
    fn code(&mut self, id: u32, raw: &str) {
        let Some(item) = self.item(id) else { return };
        let code = lines::wire_decode(raw);
        if item.kind == ToolKind::Edit {
            item.files = lines::patch_files(&code)
                .into_iter()
                .map(|(path, add, del)| FileCount { path, add: add as u32, del: del as u32 })
                .collect();
        }
        item.code = Some(cap(&code));
    }

    /// `tool_result`: a failed bash's exit code and its first error line,
    /// its output. Its state and duration are `tool_finished`'s, the line
    /// before it; a call still running without one (an older transcript)
    /// ends here (`ms` 0: a replay, no duration).
    fn result(&mut self, id: u32, ok: bool, preview: &str, ms: u64) {
        let Some(item) = self.item(id) else { return };
        let out = lines::wire_decode(preview);
        if item.state == ToolState::Run {
            item.state = if ok { ToolState::Ok } else { ToolState::Err };
            item.ms = words::tool_ms(item.at_ms, ms);
        }
        if !ok {
            item.exit = lines::exit_code(&out);
            item.err = lines::error_line(&out);
        }
        item.out = Some(cap(&out)).filter(|o| !o.is_empty());
    }

    fn intent(&mut self, id: u32, text: &str) {
        let Some(&(_, e, i)) = self.tools.iter().rev().find(|(t, _, _)| *t == id) else { return };
        let intent = one_line(&lines::unescape(text), 120);
        if let (Some(t), false) = (self.out[e].tools.as_mut(), intent.is_empty()) {
            t.items[i].text = intent.clone();
            t.items[i].intent = Some(intent);
        }
        self.sum(e);
    }

    fn sum(&mut self, e: usize) {
        let entry = &mut self.out[e];
        let Some(t) = entry.tools.as_mut() else { return };
        t.count = t.items.len() as u32;
        t.summary = summary(&t.items);
        entry.text = t.summary.clone();
    }

    fn line(&mut self, pos: u64, ms: u64, line: &str) {
        // a line the REPL replays has the time of the replay: its
        // thinking has no duration
        let (line, replayed) = match line.strip_prefix("history ") {
            Some(l) => (l, true),
            None => (line, false),
        };
        let prev = std::mem::replace(&mut self.last_ms, if replayed { 0 } else { ms });
        let you = self.you.take();
        let rec = lines::read(line);
        match rec {
            Rec::Empty | Rec::Idle | Rec::Fact => return,
            Rec::Raw(_) => {}
            _ => self.cont = None,
        }
        // his messages' marks (G1, the TUI's rule): a turn's start reads
        // what he sent since the last one
        match lines::mark_of(&rec) {
            Some(m @ Mark::Turn) => {
                let turn = self.turn.min(self.out.len());
                lines::deliver(&mut self.out[turn..], &m);
                self.turn = self.out.len();
            }
            Some(m) => {
                lines::deliver(&mut self.out, &m);
            }
            None => {}
        }
        if let Some(m) = lines::tool_move(&rec) {
            return self.tool_move(pos, if replayed { 0 } else { ms }, m);
        }
        match rec {
            Rec::Obs(o) => self.obs(pos, ms, o, if replayed { 0 } else { prev }),
            Rec::Rejected(r) => self.notice(pos, ms, words::rejected(&r)),
            Rec::Hub(h) => self.hub(pos, ms, h, you),
            // a message's next line
            Rec::Raw(line) => self.more(&line),
            _ => {}
        }
    }

    fn obs(&mut self, pos: u64, ms: u64, o: Obs, prev: u64) {
        match o {
            // one line, one entry (pos is the entry's key): the thinking
            // rides on the reply it comes before; a reply with nothing
            // visible (a tool-call-only turn) is a thinking entry alone
            Obs::Assistant(t) => {
                let (thinking, vis) = match lines::split_thinking(&t) {
                    Some((think, vis)) => {
                        let ms_thought = words::thought_ms(prev, ms);
                        (Some(Thinking { ms: ms_thought, text: lines::unescape(&think).trim().to_string() }), vis)
                    }
                    None => (None, t),
                };
                let text = lines::unescape(&vis).trim().to_string();
                let mut e = match (&thinking, text.is_empty()) {
                    (_, false) => Entry::new(pos, ms, EntryKind::Agent, text),
                    (Some(th), true) => Entry::new(pos, ms, EntryKind::Thinking, words::thought_for(th.ms)),
                    (None, true) => return,
                };
                e.thinking = thinking;
                self.push(e);
            }
            Obs::CompactionStarted => {
                self.push(Entry::new(pos, ms, EntryKind::Compacting, "compacting".into()));
            }
            Obs::CompactionDone(t) => {
                self.push(Entry::new(pos, ms, EntryKind::Compacted, lines::unescape(&t)));
            }
            // R12: a failed turn is its own entry (why, as the runtime
            // said it), its text the TUI's words for it
            Obs::TurnDone(lines::TurnEnd::Failed(why)) => {
                let text = words::turn_failed(&why, self.ctx.provider).text;
                let mut e = Entry::new(pos, ms, EntryKind::TurnFailed, text);
                e.turn_failed = Some(TurnFailed { why });
                self.push(e);
            }
            o => {
                if let Some(n) = words::obs_notice(&o, self.ctx.provider) {
                    self.notice(pos, ms, n);
                }
            }
        }
    }

    fn approval(&mut self, pos: u64, ms: u64, ok: bool, text: String, note: String, card: Option<u64>) {
        let mut e = Entry::new(pos, ms, EntryKind::Approval, text.clone());
        e.approval = Some(ApprovalFold { ok, text, note, images: Vec::new(), files: Vec::new(), card });
        self.push(e);
    }

    fn scheduled(&mut self, pos: u64, ms: u64, s: Scheduled) {
        let mut e = Entry::new(pos, ms, EntryKind::Scheduled, s.head.clone());
        e.scheduled = Some(s);
        self.push(e);
    }

    fn notice(&mut self, pos: u64, ms: u64, n: Notice) {
        let mut e = Entry::new(pos, ms, EntryKind::Notice, n.text.clone());
        e.notice = Some(n);
        self.push(e);
    }

    fn hub(&mut self, pos: u64, ms: u64, h: Hub, you: Option<usize>) {
        match h {
            Hub::You(text) => {
                let i = self.push(Entry::new(pos, ms, EntryKind::You, text));
                (self.cont, self.you) = (Some(i), Some(i));
            }
            // his fn context: on the 'you' line right before it, else
            // dropped (a stray one never lands on another entry)
            Hub::Context(raw) => {
                if let (Some(i), Ok(c)) = (you, serde_json::from_str::<FnContext>(&raw)) {
                    self.out[i].context = Some(c);
                }
            }
            // a scheduled task's run reads as its line; the note of a
            // stop is for the agent only (site/m/timers, the TUI's rule)
            // main's note of an answer to its own card: its route line
            // (the "you answered" row) says it, his answer whole
            Hub::MsgIn { from, body, .. } if lines::is_hub_sender(&from) && lines::is_main_answer_note(&body) => {}
            Hub::MsgIn { from, body, .. } if lines::is_hub_sender(&from) && (scheduled::run_line(&body).is_some() || lines::is_stop_note(&body)) => {
                if let Some(s) = scheduled::run_line(&body) {
                    self.scheduled(pos, ms, s);
                }
            }
            // G4: its message id; G5: an old direct reply (`@from`) was
            // written to him
            Hub::MsgIn { from, id, body } => {
                let mut e = Entry::new(pos, ms, EntryKind::FromAgent, body);
                e.to_you = from.starts_with('@');
                e.from = Some(from.trim_start_matches('@').to_string());
                e.msg = id.strip_prefix("m_").and_then(|n| n.parse().ok());
                self.cont = Some(self.push(e));
            }
            // what this task sent (sb-core's `sent` line)
            Hub::Sent { to, id, ask, body } => {
                let mut e = Entry::new(pos, ms, EntryKind::ToAgent, body);
                e.to = Some(to).filter(|t| !t.is_empty());
                e.asks = ask;
                e.msg = id.strip_prefix("m_").and_then(|n| n.parse().ok());
                self.cont = Some(self.push(e));
            }
            // an interrupt (sb-core writes it in the interrupt's step)
            Hub::Stopped(text) => {
                self.push(Entry::new(pos, ms, EntryKind::Stopped, text));
            }
            // BISE-86: his message didn't reach `to`; no cid here (the
            // hub's error to his window carries it, architect m_10348)
            Hub::Undelivered { to, text } => {
                let mut e = Entry::new(pos, ms, EntryKind::NotDelivered, text.clone());
                e.not_delivered = Some(NotDelivered { to, text });
                self.push(e);
            }
            // G5: an agent writing to him (level 2 in the TUI): the agent
            // entry it always was, now saying who and to him
            Hub::MsgYou { from, body } => {
                let mut e = Entry::new(pos, ms, EntryKind::Agent, body);
                e.to_you = true;
                e.from = Some(lines::shown_name(&from));
                self.cont = Some(self.push(e));
            }
            // `#3 question @docs : text`; a gate's confirm is the tool row's
            Hub::Card { id: Some(id), kind, body, text } if kind != "confirm" => {
                let (q, options) = question(&body);
                // `#3 question @docs : …`: who asked, the head's third word
                let head = text.split_once(" : ").map_or(text.as_str(), |(h, _)| h);
                let agent = head.split_whitespace().nth(2).and_then(|w| w.strip_prefix('@')).map(str::to_string);
                let answered = !self.ctx.open_cards.contains(&id);
                let card = EntryCard { id, question: q.clone(), options, answered, kind: Some(kind), agent };
                let mut e = Entry::new(pos, ms, EntryKind::Card, q);
                e.card = Some(card);
                self.cont = Some(self.push(e));
            }
            // site/m/artifacts D: `± 3 files +42 −18`
            Hub::Landed { agent, target, from, sha, files, add, del } => {
                let mut e = Entry::new(pos, ms, EntryKind::Landed, words::landed(files, add, del));
                e.landed = Some(Landed { agent, target, from, sha, files, add, del });
                self.push(e);
            }
            // pr-news (pr-design §4): what it means, never a color
            Hub::Pr { state, number, url, text } => {
                let mut e = Entry::new(pos, ms, EntryKind::Pr, text.clone());
                e.pr = Some(PrNews { number, url, text, state });
                self.push(e);
            }
            // site/m/artifacts C; a page this fold already shows (its
            // publish's page entry) comes once (architect m_10476 p7)
            Hub::Artifact { id, agent, title, kind, v } => {
                if kind == "page" && self.out.iter().any(|e| e.page.as_ref().is_some_and(|p| p.id == id)) {
                    return;
                }
                let url = if kind == "page" { (self.ctx.page)(&id).map(|p| p.url) } else { None };
                let mut e = Entry::new(pos, ms, EntryKind::Artifact, title.clone());
                e.made = Some(Made { kind_word: words::kind_word(&kind), id, agent, title, kind, v, url });
                self.push(e);
            }
            // main answered an agent for him
            // his answer as sb-core wrote it (his words with his pasted
            // files rendered in, R41): the words, images and files apart
            Hub::Answered { agent, question, answer, why } => {
                let a = (self.ctx.attached)(&answer);
                let mut e = Entry::new(pos, ms, EntryKind::Answered, a.words.clone());
                e.answered = Some(Answered { agent, question, answer: a.words, why, images: a.images, files: a.files });
                self.push(e);
            }
            // a task set or ended, read at the line's own time (a replay
            // says the same) on the hub's clock
            Hub::Scheduled(json) => {
                if let Some(s) = scheduled::hub_line(&json, ms, self.ctx.offset) {
                    self.scheduled(pos, ms, s);
                }
            }
            // a gate's card answered, folded (approvals-design.md §9)
            Hub::Approval { how, who, what, note } => {
                let (ok, text, note) = words::approval(&how, &who, &what, &note);
                self.approval(pos, ms, ok, text, note, None);
            }
            // an answer to an item, its fold line (BISE-305/307); the item
            // it answers, so a client that folded it itself skips it.
            // His answer as written, with his pasted files rendered in (R41,
            // designer m_14030 red a): the words fold, the images and files
            // ride apart, never a marker in the row
            Hub::Route { who, card, said } => {
                let a = (self.ctx.attached)(&said);
                let (text, note) = words::answered_split(&who, &a.words, self.ctx.width);
                self.approval(pos, ms, true, text, note, Some(card));
                if let Some(f) = self.out.last_mut().and_then(|e| e.approval.as_mut()) {
                    (f.images, f.files) = (a.images, a.files);
                }
            }
            // agent to agent in main's thread (the window groups the run);
            // the hub's own timer wakes and stop notes are the agents'
            Hub::Msg { from, to, id, body } => {
                if lines::is_hub_sender(&from) && (lines::timer_wake(&body).is_some() || lines::is_stop_note(&body)) {
                    return;
                }
                let mut e = Entry::new(pos, ms, EntryKind::FromAgent, body);
                e.from = Some(lines::shown_name(&from));
                e.to = Some(lines::shown_name(&to)).filter(|t| !t.is_empty());
                e.msg = id.strip_prefix("m_").and_then(|n| n.parse().ok());
                self.cont = Some(self.push(e));
            }
            // a warning, a spawn, computer use, a direct message
            h => {
                if let Some(n) = words::hub_notice(&h) {
                    self.notice(pos, ms, n);
                }
            }
        }
    }

    fn more(&mut self, line: &str) {
        let Some(i) = self.cont else { return };
        let e = &mut self.out[i];
        e.text = format!("{}\n{line}", e.text);
        if let Some(c) = e.card.as_mut() {
            let (q, options) = question(&format!("{}\n{line}", c.question));
            c.question = q.clone();
            c.options.extend(options);
            e.text = q;
        }
    }
}

/// His messages' marks, by [`lines::deliver`] (G1).
impl Delivered for Entry {
    fn yours(&self) -> Option<(&str, Delivery)> {
        (self.kind == EntryKind::You).then_some(())?;
        Some((self.text.as_str(), self.delivery?))
    }

    fn set_mark(&mut self, to: Delivery) {
        if self.kind == EntryKind::You {
            self.delivery = Some(to);
        }
    }
}

/// An agent's transcript lines as entries, oldest first.
pub fn fold(lines: &[Line], ctx: &Ctx) -> Vec<Entry> {
    let mut f = Fold { out: Vec::new(), cont: None, tools: Vec::new(), you: None, last_ms: 0, turn: 0, ctx };
    for (pos, ms, l) in lines {
        f.line(*pos, *ms, l);
    }
    f.out
}

#[cfg(test)]
#[path = "fold_tests.rs"]
mod tests;
