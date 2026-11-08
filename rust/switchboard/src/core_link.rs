//! One step of sb-core, typed (issue 25, chunk R): `Hub::core` sends an
//! input as a `bise_proto::core::In` in its `Step` envelope (the hub's
//! clock, git, the answers to its queries so far), answers sb-core's git
//! queries, and runs the effects it returns, each read as a
//! `bise_proto::core::Fx` through serde (core_fx.rs turns the plain ones
//! into `Effect`s; the ones on the hub's state are here). An effect this
//! hub does not know is skipped and logged, never a crash; an `{error}`
//! answer drops its input (hub.log, main's feed once, its sender told);
//! a dead or late sb-core is restarted (`Hub::call`, core_photo.rs).

use super::core_fx::{self, Plain};
use super::*;
use bise_proto::core::body::{self, Words};
use bise_proto::core::link::{ANeed, AStep, Answer};
use bise_proto::core::{fx as cfx, Fx as CoreFx, In};

impl Hub {
    /// An agent request (issue 37 #2): one core.rs answers or acts on
    /// before sb-core's `req` step takes sb-core's `speaks` on its sender
    /// first; the rest ride the `req` step, which checks it.
    pub(super) fn agent_step(&mut self, fx: &mut Fx, env: &mut dyn Env, token: Token, from: &str, req: AgentReq, by: crate::peer::Sender) {
        if super::pure::rides_req(&req) || self.spoke_ok(fx, env, token, from, by.clone()) {
            self.agent_req(fx, env, token, from, req, by)
        }
    }

    /// sb-core's `speaks` on `by` (bend/hub/peer.bend): true, ok (its fx dropped); false, fx spoke alone.
    fn spoke_ok(&mut self, fx: &mut Fx, env: &mut dyn Env, token: Token, from: &str, by: crate::peer::Sender) -> bool {
        self.core(fx, env, None, In::Speaks(by.speaks(token, from.into(), String::new())));
        let ok = !fx.iter().any(|e| matches!(e, Effect::Spoke { ok: false, .. }));
        fx.retain(|e| !(ok && matches!(e, Effect::Spoke { .. })));
        ok
    }

    /// Run one input in sb-core: answer its git queries (the input is
    /// replayed with the answers), then apply the effects it returns.
    pub(super) fn core(&mut self, fx: &mut Fx, env: &mut dyn Env, client: Option<ClientId>, input: In) {
        // a number sb-core cannot read fails its whole input: refuse it
        // here, naming it, to whoever sent it (crate::core_num)
        if let Some(n) = crate::core_num::too_big(&input.to_value()) {
            let e = format!("refused: {} is too large a number (at most {})", n, crate::core_num::MAX);
            match (&input, client) {
                (In::Req(r), _) => fx.push(Effect::Reply { token: r.token, body: json!({"ok": false, "error": e}) }),
                (_, Some(c)) => fx.push(notice(c, &e)),
                _ => fx.push(feed(MAIN, Feed::warn(e))),
            }
            return;
        }
        // one now for the input and its re-sends (a need replays the step)
        let now = env.now();
        let mut step = input.step(now, env.is_git(), Vec::new());
        let k0 = fx.len();
        loop {
            let out = self.call(&step.to_value());
            for l in std::mem::take(&mut self.revived) {
                fx.push(feed(MAIN, Feed::warn(l)));
            }
            let Some(mut out) = out else { return dropped(fx, &step.input, client, bise_proto::words::common::core_stopped) };
            // c6: the words its effects name, filled before they decode
            let own = own_words(&out);
            self.fill_bodies(&mut out, &own);
            let s = match core_fx::answer(&out) {
                Answer::Need(ANeed { need: q }) => {
                    // sb every's wakes: their words and the local clock
                    let a = if q["q"] == "every_wake" { self.st.timers.wakes(&q, now) } else { self.query(env, &q) };
                    step.ans.push(a);
                    continue;
                }
                Answer::Error(e) => return self.refused(fx, &step.input, client, &e.error),
                Answer::Step(s) => s,
                // not a step's answer (a newer sb-core?): nothing applied
                other => return eprintln!("sb-core: not a step's answer (skipped): {}", other.to_value()),
            };
            self.error_told = false;
            return self.apply_step(fx, env, client, k0, s, &own);
        }
    }

    /// c6 (architect m_56402): the [`Body`](bise_proto::core::body::Body)
    /// fields of a step's effects (a feed record's text or parts, the
    /// message of `sb wait`'s reply) made the words they name, in place:
    /// `own`, the step's own messages, first (the daemon journals them
    /// after the step), else the journal (bodies.rs). An effect whose
    /// body doesn't read is logged and left as it is (its decode skips it).
    fn fill_bodies(&self, out: &mut Value, own: &BTreeMap<u64, Words>) {
        let mut errors = Vec::new();
        let st = &self.st;
        let mut get = |id: u64| own.get(&id).cloned().map_or_else(|| st.words(id), Ok);
        // `get_mut`: an answer without fx (a need) stays without the key
        for f in out.get_mut("fx").and_then(Value::as_array_mut).into_iter().flatten() {
            let r = match f["fx"].as_str() {
                Some("feed") => body::fill(&bise_proto::core::Feed::TABLE, &mut f["rec"], &mut get),
                _ => body::fill(&CoreFx::TABLE, f, &mut get),
            };
            errors.extend(r.err());
        }
        for e in errors {
            self.log(&format!("sb-core: an effect's body (skipped): {e}"));
        }
    }

    /// sb-core answered `{error}`: the input is dropped, never a panic.
    /// hub.log says each; main's feed the first of a run (no flood when
    /// every Tick fails); whoever sent it gets the plain words.
    fn refused(&mut self, fx: &mut Fx, input: &In, client: Option<ClientId>, error: &str) {
        let what = input.to_value()["t"].as_str().unwrap_or("?").to_string();
        self.log(&format!("sb-core refused an input ({}): {}; dropped", what, one_line(error)));
        if !std::mem::replace(&mut self.error_told, true) {
            let w = format!("sb-core could not run an input ({}): it was dropped. hub.log has each one", what);
            fx.push(feed(MAIN, Feed::warn(w)));
        }
        dropped(fx, input, client, bise_proto::words::common::core_refused);
    }

    /// One call to sb-core. It died: restart it on the journal and run
    /// the input again, once; dead again on it, the input is dropped
    /// (None) and sb-core restarted once more (BISE-292). Late: a death
    /// too, but the input is dropped at once (a retry would hold the
    /// loop as long again). Given up: None, sb-core no longer called.
    pub(super) fn call(&mut self, v: &Value) -> Option<Value> {
        if self.gave_up.is_some() {
            return None;
        }
        // its share of the loop's message (spent.rs), a restart included
        let _t = self.spent.part("sb-core");
        let e = match self.link.call(v) {
            Ok(out) => return Some(out),
            Err(e) => e,
        };
        if let CoreError::Late { .. } = e {
            self.restart_core(&format!("{}, that input dropped", e));
            return None;
        }
        self.restart_core(&e.to_string());
        if self.gave_up.is_some() {
            return None;
        }
        match self.link.call(v) {
            Ok(out) => Some(out),
            Err(e) => {
                let what = v["t"].as_str().unwrap_or("?").to_string();
                self.restart_core(&format!("{}; stopped twice on the same input ({}): that input dropped", e, what));
                None
            }
        }
    }

    /// A committed step's answer: its view into the mirror, then its
    /// effects. A view that doesn't decode is logged (load_view): the
    /// effects still run, the journal's included.
    /// `own`: the words of the step's own messages (a delivery's text).
    pub(super) fn apply_step(&mut self, fx: &mut Fx, env: &mut dyn Env, client: Option<ClientId>, k0: usize, s: AStep, own: &BTreeMap<u64, Words>) {
        if s.dirty {
            self.dirty = true;
        }
        // his message's id, typed by sb-core (A1): the first one counts
        self.said = self.said.or(s.msg);
        // his answer to a card, typed by sb-core (inbox picks (3))
        self.answered = self.answered.take().or_else(|| bise_proto::rpc::CardAnswer::of_step(&s));
        // the state after the step first: the deliveries render from it
        self.load_view(&s.view);
        for f in s.fx {
            match core_fx::read(f) {
                Plain::Effect(e) => fx.push(e),
                Plain::Nothing => {}
                Plain::Hub(f) => self.effect(fx, env, client, f, own),
                Plain::Skip(why) => eprintln!("{}", why),
            }
        }
        // event-wake: a watch that ended (but by a hit, whose facts
        // its input carries: core_wake.rs) gets its thread's line
        self.wake_lines(fx, k0, None);
        // a timer without a name: its small-model call (every_name.rs)
        if let Some((id, request)) = self.timer_names.next(&self.st.timers) {
            fx.push(Effect::AskTimerName { id, request });
        }
    }

    /// A git query of sb-core (RFC 0002), on the workspace the mirror
    /// knows for the task.
    fn query(&mut self, env: &mut dyn Env, q: &Value) -> Value {
        let ws: Option<Workspace> = serde_json::from_value(q["ws"].clone()).ok();
        // its share of the loop's message, by query (spent.rs)
        let _t = self.spent.part(match jstr(q, "q").as_str() {
            "worktree_loss" => "git worktree_loss",
            _ => "git query",
        });
        match jstr(q, "q").as_str() {
            "worktree_loss" => {
                let l = ws.map(|w| env.worktree_loss(&w)).unwrap_or_default();
                json!({"dirty": l.dirty, "unpushed": l.unpushed})
            }
            other => json!({"err": format!("unknown query: {}", other)}),
        }
    }

    /// One effect of sb-core on the hub's state (the plain ones are
    /// core_fx.rs's).
    fn effect(&mut self, fx: &mut Fx, env: &mut dyn Env, client: Option<ClientId>, f: CoreFx, own: &BTreeMap<u64, Words>) {
        match f {
            // an event that is not an object: never in the journal, never a panic
            CoreFx::Journal(j) if !j.ev.is_object() => self.log(&format!("sb-core: a bad journal event (skipped): {}", clip(&j.ev.to_string(), 200))),
            CoreFx::Journal(j) => {
                fx.push(Effect::Journal(j.ev.clone()));
                // a timer set or ended (sb every): its ◷ lines
                self.timer_journal_lines(fx, &j.ev);
                // les bims: a message sent, a report made (core_doing.rs)
                Self::activity_of_journal(fx, &j.ev);
            }
            CoreFx::Interrupt(i) => {
                let by = self.interrupt_by.clone().unwrap_or_else(|| "bise".into());
                fx.push(Effect::Interrupt { agent: i.agent, by })
            }
            // the reply to `sb wait`: its `message`, the words fill_bodies
            // read, goes in the body here, the one place (fx.rs Reply)
            CoreFx::Reply(cfx::Reply { token, mut body, message: Some(m) }) => {
                body["message"] = json!(m);
                fx.push(Effect::Reply { token, body })
            }
            CoreFx::Reply(cfx::Reply { token, body, message: None }) => match self.spawn_asks.remove(&token) {
                // issue #4: a spawn that asked for a model: the daemon
                // picks it, writes the task's choice, then answers
                Some(ask) if body["ok"] == json!(true) && body["name"].is_string() => {
                    fx.push(Effect::SpawnModel { token, agent: jstr(&body, "name"), ask, body })
                }
                _ => fx.push(Effect::Reply { token, body }),
            },
            CoreFx::Deliver(d) => self.deliver(fx, env, d, own),
            // the user's digit on a hub item (merge.rs)
            CoreFx::CardChoice(c) => self.card_choice(fx, env, c),
            CoreFx::Notice(n) => {
                if let Some(c) = client {
                    fx.push(notice(c, &n.text));
                }
            }
            CoreFx::FocusMain(m) => {
                for (c, v) in self.clients.iter() {
                    if v.focus == m.name {
                        fx.push(Effect::ToClient {
                            client: *c,
                            body: json!({"ev": "focus", "focus": MAIN}),
                        });
                    }
                }
            }
            CoreFx::UserSent(u) => {
                if let Some(v) = client.and_then(|c| self.clients.get_mut(&c)) {
                    v.sent.push(u.text);
                }
            }
            CoreFx::Renamed(cfx::Renamed { old, new }) => {
                for v in self.clients.values_mut().filter(|v| v.focus == old) {
                    v.focus = new.clone();
                }
                fx.push(Effect::Renamed { old, new });
            }
            CoreFx::Retarget(cfx::Retarget { from, to }) => fx.push(Effect::Retarget { from, to }),
            // core_fx::plain took the others
            other => eprintln!("sb-core: effect {} not run here (skipped)", other.tag()),
        }
    }

    /// The text a delivery puts in the agent's context (core_fx::delivery),
    /// as a new turn or as steering.
    fn deliver(&mut self, fx: &mut Fx, env: &mut dyn Env, d: cfx::Deliver, own: &BTreeMap<u64, Words>) {
        let status = d.status.then(|| board::status_block(&self.st, env.now()));
        let text = |id: u64| own.get(&id).map(|w| w.text.clone()).unwrap_or_else(|| self.st.text(id));
        let msgs = d
            .msgs
            .iter()
            // the step's own messages: its view sent them whole
            .filter_map(|m| Some(prompts::tagged(self.st.msg(m.id)?, &text(m.id), &m.rel)))
            .collect();
        let text = core_fx::delivery(&d.notes, status, msgs);
        let agent = d.agent;
        if d.mode == "steer" {
            fx.push(Effect::Steer { agent, text });
        } else {
            fx.push(Effect::Say { agent, text });
        }
    }
}

/// The words of a step's own messages (its `message_sent` events), by id.
fn own_words(out: &Value) -> BTreeMap<u64, Words> {
    out["fx"].as_array().into_iter().flatten().filter(|f| f["fx"] == "journal").filter_map(|f| Words::of_journal(&f["ev"])).collect()
}

/// An input sb-core did not run: whoever sent it hears it in plain
/// `words` (a request its reply, a client a notice); a hub input, nothing.
fn dropped(fx: &mut Fx, input: &In, client: Option<ClientId>, words: &str) {
    match (input, client) {
        (In::Req(r), _) => fx.push(Effect::Reply { token: r.token, body: json!({"ok": false, "error": words}) }),
        (_, Some(c)) => fx.push(notice(c, words)),
        _ => {}
    }
}
