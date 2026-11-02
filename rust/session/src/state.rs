//! Rebuild the state from the log (§7 step 4): the last checkpoint,
//! then every event after it, in seq order.
use crate::reader::Log;
use crate::types::*;
use serde_json::{json, Value};

#[derive(Debug, Clone, PartialEq)]
pub struct State {
    /// the context events, in order; a compaction summary is its
    /// compaction_done's seq
    pub context: Vec<u64>,
    /// input_queued not yet consumed or dropped
    pub queue: Vec<u64>,
    pub system: Text,
    pub tools: Vec<ToolDef>,
    pub model: ModelRef,
    pub limits: Limits,
    pub counters: Counters,
    pub usage_total: UsageTotal,
    /// the turn of the last turn_started with no turn_ended yet
    pub open_turn: Option<u64>,
    /// seq of the last compaction_started with no done/failed yet (and its id)
    pub open_compaction: Option<(u64, u64)>,
}

impl Default for State {
    fn default() -> Self {
        State {
            context: Vec::new(),
            queue: Vec::new(),
            system: Text::Inline { text: String::new() },
            tools: Vec::new(),
            model: ModelRef::default(),
            limits: Limits::default(),
            counters: Counters::default(),
            usage_total: UsageTotal::default(),
            open_turn: None,
            open_compaction: None,
        }
    }
}

impl State {
    /// The state after the whole log.
    pub fn rebuild(log: &Log) -> State {
        let start = log
            .events
            .iter()
            .rposition(|e| matches!(e.payload, Some(Payload::Checkpoint(_))))
            .unwrap_or(0);
        let mut st = State::default();
        for e in &log.events[start..] {
            st.apply(e.seq, e.turn, e.payload.as_ref());
        }
        st
    }

    /// One event (an unknown one, payload None, changes nothing, not
    /// even the turn counter).
    pub fn apply(&mut self, seq: u64, turn: Option<u64>, p: Option<&Payload>) {
        let Some(p) = p else { return };
        if let Some(t) = turn {
            self.counters.turn = self.counters.turn.max(t);
        }
        let take_queue = |q: &mut Vec<u64>, from: Option<u64>| {
            if let Some(f) = from {
                q.retain(|&s| s != f);
            }
        };
        match p {
            Payload::Checkpoint(c) => {
                self.context = c.context.clone();
                self.queue = c.queue.clone();
                self.system = c.system.clone();
                self.tools = c.tools.clone();
                self.model = c.model.clone();
                self.limits = c.limits.clone();
                self.counters = c.counters;
                self.usage_total = c.usage_total.clone();
            }
            Payload::ContextSet(c) => {
                if let Some(s) = &c.system {
                    self.system = s.clone();
                }
                if let Some(t) = &c.tools {
                    self.tools = t.clone();
                }
            }
            Payload::LimitsSet(l) => {
                self.limits.compact_threshold = l.compact_threshold.or(self.limits.compact_threshold);
                self.limits.select_budget = l.select_budget.or(self.limits.select_budget);
                self.limits.max_nulls = l.max_nulls.or(self.limits.max_nulls);
            }
            Payload::ModelSet(m) => self.model = m.model.clone(),
            Payload::TurnStarted { .. } => self.open_turn = turn.or(Some(self.counters.turn + 1)),
            Payload::TurnEnded(t) => {
                self.open_turn = None;
                if let Some(c) = t.counts {
                    self.counters.inputs = c.inputs;
                    self.counters.actions = c.actions;
                }
            }
            Payload::UserMessage(m) => {
                take_queue(&mut self.queue, m.from_queue);
                self.context.push(seq);
            }
            Payload::ContextInjected(m) => {
                take_queue(&mut self.queue, m.from_queue);
                self.context.push(seq);
            }
            Payload::AgentMessage(m) => {
                take_queue(&mut self.queue, m.from_queue);
                self.context.push(seq);
            }
            Payload::AssistantMessage(m) => {
                self.counters.req = self.counters.req.max(m.req);
                self.context.push(seq);
            }
            Payload::ToolResult(r) => {
                // a synthetic result goes right after its call's group
                // (BISE-242), else at the end like every context event
                match r.after.and_then(|a| self.context.iter().position(|&s| s == a)) {
                    Some(p) => self.context.insert(p + 1, seq),
                    None => self.context.push(seq),
                }
            }
            Payload::Usage(u) => {
                self.counters.req = self.counters.req.max(u.req);
                self.usage_total.input += u.input;
                self.usage_total.output += u.output;
                self.usage_total.cache_read += u.cache_read.unwrap_or(0);
                self.usage_total.cache_write += u.cache_write.unwrap_or(0);
                if let Some(usd) = u.cost.as_ref().and_then(|c| c.get("usd")).and_then(Value::as_f64) {
                    self.usage_total.usd = Some(self.usage_total.usd.unwrap_or(0.0) + usd);
                }
            }
            Payload::RequestFailed(r) => self.counters.req = self.counters.req.max(r.req),
            Payload::InputQueued(_) => self.queue.push(seq),
            Payload::InputDropped(d) => self.queue.retain(|&s| s != d.queued),
            Payload::CompactionStarted(c) => {
                self.counters.compaction = self.counters.compaction.max(c.id);
                self.open_compaction = Some((seq, c.id));
            }
            Payload::CompactionFailed(_) => self.open_compaction = None,
            Payload::CompactionDone(c) => {
                self.open_compaction = None;
                self.counters.compaction = self.counters.compaction.max(c.id);
                // the range is a span of the context, from `from` to `to`
                // (the recorder resolved the Core's positions to seqs),
                // never a span of seqs: after a compaction the context is
                // out of seq order (the summary goes before the messages it
                // kept), and a seq range left those kept messages in the
                // log for good (compact-100k). A bound that is not in the
                // context (a log written by hand) falls back to the first
                // or last event inside the seqs.
                let ctx = &self.context;
                let inside = |s: u64| s >= c.replaces.from && s <= c.replaces.to;
                let a = ctx.iter().position(|&x| x == c.replaces.from).or_else(|| ctx.iter().position(|&s| inside(s)));
                let b = ctx.iter().position(|&x| x == c.replaces.to).or_else(|| ctx.iter().rposition(|&s| inside(s)));
                let (a, b) = match (a, b) {
                    (Some(a), Some(b)) if a <= b => (a, b + 1),
                    _ => (ctx.len(), ctx.len()),
                };
                let tail = self.context.split_off(b);
                let span = self.context.split_off(a);
                self.context.push(seq);
                self.context.extend(span.into_iter().filter(|s| c.kept.contains(s)));
                self.context.extend(tail);
            }
            _ => {}
        }
    }

    /// The payload of a checkpoint of this state (§4 Checkpoint).
    pub fn checkpoint(&self, upto: u64) -> Value {
        json!({
            "upto": upto, "context": self.context, "system": self.system, "tools": self.tools,
            "model": self.model, "limits": self.limits, "queue": self.queue,
            "counters": self.counters, "usage_total": self.usage_total,
        })
    }
}
