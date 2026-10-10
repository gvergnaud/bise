//! `sb wake` (event-wake) on the link to sb-core: the request's checks
//! and words (wake.rs), the watch a backgrounded bash command gets at its
//! handoff (the runtime's `bg_handoff` wire line), and the look's hit
//! (daemon/wakes.rs). The watch, its end and the wake are sb-core's
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
    pub(super) fn wake_hit(&mut self, fx: &mut Fx, env: &mut dyn Env, id: u64, text: &str) {
        self.core(fx, env, None, json!({"t": "wake_hit", "id": id, "text": text}));
    }
}
