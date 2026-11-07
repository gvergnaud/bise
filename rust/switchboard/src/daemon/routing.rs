//! bise's home hub routes his words (bise desktop S2 step 3; issue 37 #1):
//! the shell's half of it, I/O only. sb-core decides (xhub.bend
//! `addressed`, laws `route_ask_*`, `addressed_*`, `address_*`).
//!
//! His plain words to the agent in view come here before they are said:
//! [`Shell::route_input`] steps sb-core with `Input::RouteAsk` (his words,
//! and on bise's home hub only, the registry's projects: empty elsewhere,
//! so nothing routes), and reads only its answer, `Effect::RouteAsked`.
//! Routed: sb-core held them 2 s for his correction, then forwards them.
//! Not routed: the caller steps his input to the agent in view unchanged
//! (`Input::ClientInput`), in the same loop message, with no other step
//! between the two. sb-core routes only his explicit `@<project> …` (no
//! agent of this hub by that name: an agent wins); a project's name in his
//! words, the front file, page or title never route (his m_75).
//!
//! Read-only: never starts a hub, never writes.

use super::Shell;
use crate::core::{Effect, Input};

impl Shell {
    /// His input to the agent in view goes to a project when he addressed
    /// it (true: sb-core held it); false: not routed, the caller steps the
    /// input as today, next.
    /// `context`: the input's fn context as it came, carried to the
    /// project (never read here). `text`: his words (the address is read
    /// from them; they go whole, his attached files are his parts). `parts`: his
    /// message's parts (msg-parts JSON, "" none), held and forwarded with
    /// his words (R41 (6)). His words go whole, `@<project>` included.
    #[allow(clippy::too_many_arguments)]
    pub(super) fn route_input(&mut self, focus: &str, text: &str, via: &str, queued: bool, context: Option<&serde_json::Value>, parts: &str) -> bool {
        let projects = match crate::paths::is_home(&self.opts.paths.workspace) {
            true => {
                let rows = bise_home::projects::list(&self.opts.home, &crate::paths::home_workspace());
                rows.into_iter().filter(|r| !r.home).map(|r| (r.id, r.name)).collect()
            }
            false => Vec::new(),
        };
        let ask = Input::RouteAsk { focus: focus.into(), text: text.into(), via: via.into(), queued, context: context.cloned(), parts: parts.into(), projects };
        let fx = self.hub.handle(ask, &mut self.env);
        let routed = fx.iter().any(|e| matches!(e, Effect::RouteAsked { routed: true }));
        self.step_fx(fx);
        routed
    }
}
