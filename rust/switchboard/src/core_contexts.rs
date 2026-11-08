//! The hub's side of the agents' contexts (contexts.rs, send-fast,
//! architect m_35501): an input marks them pending; a step that delivers
//! to a REPL (a say, a steer) builds them at once and puts their
//! `Context` effects before its first delivery, so a REPL never takes an
//! input on a context older than it; else the shell's loop builds them
//! ([`Hub::contexts_flush`]) when its gate says so, and before any line it
//! writes to a REPL itself (daemon/contexts.rs).

use super::{Effect, Fx, Hub};

impl Hub {
    /// At every input (Hub::handle): the contexts are pending; built now
    /// when the step delivers to a REPL, their effects first.
    pub(super) fn refresh_contexts(&mut self, fx: &mut Fx, now: u64) {
        self.contexts.pend();
        if let Some(at) = fx.iter().position(|e| matches!(e, Effect::Say { .. } | Effect::Steer { .. })) {
            let built = self.contexts_flush(now);
            fx.splice(at..at, built);
        }
    }

    /// The pending contexts built: the changed ones as `Context` effects
    /// (none when nothing is pending).
    pub fn contexts_flush(&mut self, now: u64) -> Fx {
        let texts = self.contexts.flush(&self.st, now, &self.models);
        texts.into_iter().map(|(agent, text)| Effect::Context { agent, text }).collect()
    }

    /// An input came since the last build.
    pub fn contexts_pending(&self) -> bool {
        self.contexts.pending()
    }
}

#[cfg(test)]
mod tests {
    use super::super::tests::T;
    use crate::core::{AgentReq, Effect, Input};
    use crate::model::{Brief, MAIN};

    fn spawn_req(t: &mut T, name: &str) -> Vec<Effect> {
        let brief = Brief { objective: format!("objective of {name}"), ..Brief::default() };
        let req = AgentReq::Spawn { name: name.into(), brief, worktree: false, with_changes: false, place: String::new(), feature: String::new(), ask: Default::default(), attach: Vec::new(), parts: String::new(), fwd: None };
        t.req(MAIN, req).1
    }

    fn contexts(fx: &[Effect]) -> Vec<&str> {
        fx.iter().filter_map(|e| if let Effect::Context { agent, .. } = e { Some(agent.as_str()) } else { None }).collect()
    }

    /// Law (b): a step that delivers to no REPL builds no context; they
    /// stay pending.
    #[test]
    fn a_step_without_delivery_leaves_the_contexts_pending() {
        let mut t = T::new();
        t.hub.contexts_flush(0);
        let fx = spawn_req(&mut t, "docs");
        assert!(!fx.iter().any(|e| matches!(e, Effect::Say { .. } | Effect::Steer { .. })), "{fx:?}");
        assert!(contexts(&fx).is_empty(), "{fx:?}");
        assert!(t.hub.contexts_pending());
    }

    /// Law (a): a step that delivers to a REPL carries the pending
    /// contexts before its first delivery (the task's brief: its own
    /// context and main's board with it are written before the say).
    #[test]
    fn a_delivery_goes_after_the_new_contexts() {
        let mut t = T::new();
        spawn_req(&mut t, "docs");
        let fx = t.go(Input::ReplReady { agent: "docs".into() });
        let say = fx.iter().position(|e| matches!(e, Effect::Say { agent, .. } if agent == "docs")).expect("the brief");
        let ctx = fx.iter().position(|e| matches!(e, Effect::Context { agent, .. } if agent == "docs")).expect("docs' context");
        assert!(ctx < say, "{fx:?}");
        assert!(contexts(&fx[..say]).contains(&MAIN), "main's board with docs, before: {fx:?}");
        assert!(!t.hub.contexts_pending());
    }

    /// Law: after any number of steps with no delivery, a flush gives
    /// each agent the text a fresh build gives.
    #[test]
    fn flushed_equals_fresh() {
        let mut t = T::new();
        for n in ["a", "b", "c"] {
            spawn_req(&mut t, n);
        }
        let now = t.env.now;
        t.hub.contexts_flush(now);
        let fresh = crate::contexts::fresh(&t.hub.st, now, &t.hub.models);
        assert!(!fresh.is_empty());
        for (name, text) in fresh {
            assert_eq!(t.hub.contexts.text(&name), Some(text.as_str()), "{name}");
        }
    }
}
