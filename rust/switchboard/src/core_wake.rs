//! `sb wake` (event-wake) on the link to sb-core: the request's checks
//! and words (wake.rs), the watch a backgrounded bash command gets at its
//! handoff (the runtime's `bg_handoff` wire line), and the look's hit
//! (daemon/wakes.rs), and the thread's `sb wake` line of each watch that
//! ended (`wake_lines`, an effect after sb-core's `wake_end`, like the
//! timers' `scheduled` lines). The watch, its end and the wake are sb-core's
//! (`wake_set`, `wake_stop`, `wake_hit` inputs; bend/hub/wakes.bend).

use super::*;
use crate::wake::{self, Spec};

impl Hub {
    pub fn wakes(&self) -> &wake::Wakes {
        &self.st.wakes
    }

    /// One watch for `agent`; its id (sb-core's `wake_set` journal line),
    /// or None when sb-core set none (no such active agent).
    fn wake_set(&mut self, fx: &mut Fx, env: &mut dyn Env, agent: &str, spec: &Spec, max_ms: u64, quiet: bool) -> Option<u64> {
        let max_text = if quiet { String::new() } else { wake::max_text(spec, max_ms) };
        let k = fx.len();
        self.core(
            fx,
            env,
            None,
            json!({"t": "wake_set", "agent": agent, "spec": spec.json(), "max_text": max_text, "max_ms": max_ms, "quiet": quiet}),
        );
        fx[k..].iter().find_map(|e| match e {
            Effect::Journal(j) if j["type"] == "wake_set" => j["watch"]["id"].as_u64(),
            _ => None,
        })
    }

    /// `sb wake`: list, stop or set one of the caller's own watches; the reply.
    pub(super) fn wake_req(&mut self, fx: &mut Fx, env: &mut dyn Env, from: &str, r: WakeReq) -> Value {
        let Some(by) = self.st.resolve(from) else {
            return json!({"ok": false, "error": format!("unknown agent: {}", from)});
        };
        match r {
            WakeReq::List => json!({"ok": true, "text": self.st.wakes.list(&by, env.now())}),
            WakeReq::StopBg(slot) => match self.st.wakes.of_bg(&by, &slot) {
                Some(id) => self.wake_req(fx, env, from, WakeReq::Stop(id)),
                None => json!({"ok": false, "error": format!("no watch on background {} (it ended, or `sb wake` lists yours)", slot)}),
            },
            WakeReq::Stop(id) => {
                if self.st.wakes.live.get(&id).is_none_or(|w| w.agent != by) {
                    return json!({"ok": false, "error": format!("no watch #{} of yours (`sb wake` lists them)", id)});
                }
                self.core(fx, env, None, json!({"t": "wake_stop", "id": id, "why": format!("stopped by {}", by)}));
                json!({"ok": true, "text": format!("watch #{} stopped", id)})
            }
            WakeReq::Add { spec, max_ms } => match self.wake_set(fx, env, &by, &spec, max_ms, false) {
                Some(id) => json!({"ok": true, "id": id, "text": wake::set_text(id, &spec, max_ms)}),
                None => json!({"ok": false, "error": format!("sb wake: @{} is not active", by)}),
            },
        }
    }

    /// The bash tool handed a command off to the background (its wire
    /// line `bg_handoff : {"slot", "cmd"}`): a quiet watch on its slot,
    /// default on (`sb wake --stop <id>` to not be woken).
    pub(super) fn bg_handoff(&mut self, fx: &mut Fx, env: &mut dyn Env, agent: &str, args: &str) {
        let Ok(v) = serde_json::from_str::<Value>(args) else { return };
        let slot = v["slot"].as_str().unwrap_or_default();
        if slot.is_empty() || !self.st.agents.contains_key(agent) {
            return;
        }
        let spec = Spec::bg(slot, v["cmd"].as_str().unwrap_or_default());
        self.wake_set(fx, env, agent, &spec, wake::BG_MAX_MS, true);
    }

    /// The look saw watch `id`'s event (daemon/wakes.rs): sb-core ends it
    /// and wakes its agent, once.
    pub(super) fn wake_hit(&mut self, fx: &mut Fx, env: &mut dyn Env, id: u64, text: &str, hit: wake::Hit) {
        let k = fx.len();
        self.core(fx, env, None, json!({"t": "wake_hit", "id": id, "text": text}));
        self.wake_lines(fx, k, Some(&hit));
    }

    /// The thread's line of each watch that ended in `fx[k..]` (sb-core's
    /// `wake_end` journal events), `sb wake : <json>` (wake.rs
    /// `end_line`), in its agent's thread: a hit's only with `hit` (its
    /// facts, from wake_hit), the others without; bise's message sent to
    /// that agent in the same step is the one that woke it (its id rides
    /// on the line, so the thread folds it in). Effects only: a replay
    /// writes none.
    pub(super) fn wake_lines(&self, fx: &mut Fx, k: usize, hit: Option<&wake::Hit>) {
        let ends: Vec<(u64, String)> = fx[k..]
            .iter()
            .filter_map(|e| match e {
                Effect::Journal(j) if j["type"] == "wake_end" => Some((j["id"].as_u64()?, j["why"].as_str().unwrap_or_default().to_string())),
                _ => None,
            })
            .filter(|(_, why)| (why == "hit") == hit.is_some())
            .collect();
        for (id, why) in ends {
            let Some(e) = self.st.wakes.ended.iter().rev().find(|e| e.watch.id == id) else { continue };
            let agent = &e.watch.agent;
            let msg = fx[k..].iter().find_map(|f| match f {
                Effect::Journal(j) if j["type"] == "message_sent" && j["msg"]["to"] == agent.as_str() && j["msg"]["from"] == HUB => j["msg"]["id"].as_u64(),
                _ => None,
            });
            if let Some(l) = wake::end_line(&e.watch, &why, e.at, hit, msg) {
                fx.push(line(agent, "wake", &serde_json::to_string(&l).unwrap_or_default()));
            }
        }
    }
}
