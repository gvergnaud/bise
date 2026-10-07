//! What bise ambient's capsule adds to an input for main: the line main's
//! rule reads to answer in one short line (capsule_hint) and the user's
//! open steps (steps_hint, read from the page cards). Moved out of
//! daemon.rs unchanged (architect m_12143).

use super::*;

impl Shell {
    /// A client's `input` (after its keys are reloaded): a step of his
    /// answered on its card, an answer to a page's question, words bise's
    /// home hub routes to a project, else an input with its files, the
    /// capsule's hint and his open steps. Moved out of daemon.rs's input
    /// arm unchanged (architect m_12280).
    pub(super) fn input(&mut self, id: ClientId, v: &Value) {
        let s = |k: &str| v.get(k).and_then(|x| x.as_str()).unwrap_or("").to_string();
        // an answer to a page's question: its words, for the page
        // from the capsule: a hint main's rule reads (one short line
        // back); never on an answer or a slash command
        // a step of his answered on its card (roadmap D): ticked
        // on its page, the page's agent told there
        if !self.page_step_answer(id, &s("text")) {
            let queued = v.get("queued").and_then(|x| x.as_bool()).unwrap_or(false);
            let text = match self.page_reply_text(&s("text")) {
                Some(reply) => reply,
                None => {
                    // his attached files, rendered once (item H)
                    let said = fn_context::with_files(v, s("text"));
                    // bise's home hub: words about a project go there (daemon/routing.rs)
                    if self.route_input(&s("focus"), &s("text"), &said, &s("via"), queued, v.get("context")) {
                        return self.page_answers();
                    }
                    let t = Self::capsule_hint(said, &s("via"));
                    self.steps_hint(t, &s("via"))
                }
            };
            // S9: with its fn context (daemon/fn_context.rs)
            self.step_input(id, v, s("focus"), text, queued);
        }
        self.page_answers();
    }

    /// An input from bise ambient's capsule (`via: "capsule"`) carries one
    /// more line, the one main's rule reads to answer in one short line
    /// (prompts.rs); a slash command (`/answer N 1`) never gets it.
    pub(super) fn capsule_hint(text: String, via: &str) -> String {
        if text.trim_start().starts_with('/') {
            return text;
        }
        match via {
            "capsule" => format!("{text}\n\n[from the capsule: answer in one short line]"),
            // fn space's typed text sent with tab (ambient-lead m_6513):
            // main starts an agent on it, only main does
            "capsule-start" => format!("{text}\n\n[from the capsule: start an agent; answer in one short line]"),
            _ => text,
        }
    }

    /// The user's open steps (roadmap D) as one more line of a capsule
    /// message to main: "done with the payment" from anywhere ticks the
    /// right row (main runs `sb page tick`). Unchanged without any.
    pub(super) fn steps_hint(&mut self, text: String, via: &str) -> String {
        if via != "capsule" || text.trim_start().starts_with('/') {
            return text;
        }
        let Some(pages) = self.pg.pages.clone() else { return text };
        let keys: Vec<(String, String)> = self.page_cards().values().filter(|(_, k)| k.starts_with("row:")).cloned().collect();
        let mut steps: Vec<String> = Vec::new();
        for (id, key) in keys {
            let Some((block, item)) = key.trim_start_matches("row:").split_once('/') else { continue };
            if let Some(s) = pages.store.steps(&id).into_iter().find(|s| s.block == block && s.item == item && s.reply.is_none()) {
                steps.push(format!("{id} {item} \"{}\"", s.text));
            }
        }
        if steps.is_empty() {
            return text;
        }
        format!("{text}
[his open steps: {}; when he says one is done: sb page tick <page> <item>]", steps.join(", "))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The capsule's input carries the line main's rule reads (prompts.rs);
    /// any other client's input, and a slash command, stay as they are.
    #[test]
    fn an_input_from_the_capsule_carries_the_hint_main_reads() {
        let hinted = Shell::capsule_hint("what's running?".into(), "capsule");
        assert!(hinted.starts_with("what's running?\n\n"));
        assert!(hinted.ends_with("[from the capsule: answer in one short line]"));
        // main's rule names the same line (prompts.rs)
        assert!(crate::prompts::main_role("/w", "/t", "", true).contains("a `[from the capsule…]` line"));
        assert_eq!(Shell::capsule_hint("what's running?".into(), ""), "what's running?");
        assert_eq!(Shell::capsule_hint("hi".into(), "tui"), "hi");
        assert_eq!(Shell::capsule_hint("/answer 3 1".into(), "capsule"), "/answer 3 1");
        // fn space + tab: main starts an agent on it (ambient-lead m_6513)
        assert_eq!(
            Shell::capsule_hint("start an agent for: fix the login loop".into(), "capsule-start"),
            "start an agent for: fix the login loop\n\n[from the capsule: start an agent; answer in one short line]"
        );
    }
}
