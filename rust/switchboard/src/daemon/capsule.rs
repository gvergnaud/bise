//! What bise ambient's capsule adds to an input for main: the line main's
//! rule reads to answer in one short line (capsule_hint) and the user's
//! open steps (steps_hint, read from the page cards). Moved out of
//! daemon.rs unchanged (architect m_12143). Both go as hidden hint parts
//! after his own (bise_proto::parts::Part::Hint, ambient-lead m_22706):
//! the model gets the same text, his bubble never shows them. Its input
//! handler says what it did with his words ([`Took`]: a page took them,
//! routed, stepped), which turn/send's result reads (send-reason).

use super::page_cards::Page;
use super::*;
use bise_proto::parts::Part;

/// What [`Shell::input`] did with his words (send-reason: turn/send's
/// result says it, daemon/proto/send.rs).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Took {
    /// a page's question answered whole there (`Page::Done`)
    Page,
    /// words for another project, held for it (`route_input`)
    Routed,
    /// stepped by sb-core: a message of his, or a command it ran
    Stepped,
}

impl Shell {
    /// His words (turn/send, command/run's non-command line: proto/cmds.rs
    /// proto_input, the one caller since the wire's `input` op went in
    /// client-protocol P3b; its keys reloaded first): a step of his
    /// answered on its card, an answer to a page's question, words bise's
    /// home hub routes to a project, else an input with its files, the
    /// capsule's hint and his open steps. Moved out of daemon.rs's input
    /// arm unchanged (architect m_12280).
    /// What it did ([`Took`]): turn/send's result says it (send-reason).
    pub(super) fn input(&mut self, id: ClientId, v: &Value) -> Took {
        let took = self.input_took(id, v);
        self.page_answers();
        took
    }

    fn input_took(&mut self, id: ClientId, v: &Value) -> Took {
        let s = |k: &str| v.get(k).and_then(|x| x.as_str()).unwrap_or("").to_string();
        // an answer to a page's question: its words, for the page
        // from the capsule: a hint main's rule reads (one short line
        // back); never on an answer or a slash command
        // a step of his answered on its card (roadmap D): ticked
        // on its page, the page's agent told there
        let page = self.page_first(id, &s("text"), &[]);
        let queued = v.get("queued").and_then(|x| x.as_bool()).unwrap_or(false);
        let text = match page {
            Page::Done => return Took::Page,
            Page::Reply(reply) => reply,
            Page::Not => {
                // his attached files are his parts already (proto_input)
                let said = s("text");
                // bise's home hub: words he addressed to a project (`@<project> …`) go there (daemon/routing.rs)
                // his parts (msg-parts) go with his routed words (R41 (6))
                if self.route_input(&s("focus"), &said, &s("via"), queued, v.get("context"), &s("parts")) {
                    return Took::Routed;
                }
                // the capsule's lines for the model, as hidden hint
                // parts after his own (ambient-lead m_22706, architect
                // m_22715): the model gets the same bytes, his bubble
                // none of them
                let mut hints: Vec<Part> = Self::capsule_hint(&said, &s("via")).into_iter().collect();
                hints.extend(self.steps_hint(&said, &s("via")));
                // a plugin he picked with `@` (at-plugins): its line, from his typed part
                hints.extend(self.plugin_hints(&s("focus"), &s("parts")));
                if !hints.is_empty() {
                    let (text, parts) = hinted(&said, &s("parts"), hints);
                    let mut v = v.clone();
                    v["parts"] = json!(parts);
                    self.step_input(id, &v, s("focus"), text, queued);
                    return Took::Stepped;
                }
                said
            }
        };
        // S9: with its fn context (daemon/fn_context.rs)
        self.step_input(id, v, s("focus"), text, queued);
        Took::Stepped
    }

    /// An input from bise ambient's capsule (`via: "capsule"`) carries one
    /// more line, the one main's rule reads to answer in one short line
    /// (prompts.rs; fn space + tab, `capsule-start`: main starts an agent
    /// on it, ambient-lead m_6513); a slash command (`/answer N 1`) never
    /// gets it. The line itself: bise_proto::parts::capsule_hint.
    pub(super) fn capsule_hint(text: &str, via: &str) -> Option<Part> {
        match text.trim_start().starts_with('/') {
            true => None,
            false => bise_proto::parts::capsule_hint(via),
        }
    }

    /// The user's open steps (roadmap D) as one more line of a capsule
    /// message to main: "done with the payment" from anywhere ticks the
    /// right row (main runs `sb page tick`). None without any. The line
    /// itself: bise_proto::parts::steps_hint.
    pub(super) fn steps_hint(&mut self, text: &str, via: &str) -> Option<Part> {
        if via != "capsule" || text.trim_start().starts_with('/') {
            return None;
        }
        let pages = self.pg.pages.clone()?;
        let keys: Vec<(String, String)> = self.page_cards().values().filter(|(_, k)| k.starts_with("row:")).cloned().collect();
        let mut steps: Vec<String> = Vec::new();
        for (id, key) in keys {
            let Some((block, item)) = key.trim_start_matches("row:").split_once('/') else { continue };
            if let Some(s) = pages.store.steps(&id).into_iter().find(|s| s.block == block && s.item == item && s.reply.is_none()) {
                steps.push(format!("{id} {item} \"{}\"", s.text));
            }
        }
        bise_proto::parts::steps_hint(&steps)
    }
}

impl Shell {
    /// One hidden line per plugin he picked with `@` (`Part::Plugin` in his
    /// parts, at-plugins, architect m_53238): its skill and tools, or that
    /// it is off or not here (crate::plugin_hint). The plugins of the
    /// agent's own workspace (a task's worktree), resolved only when his
    /// message holds such a part.
    fn plugin_hints(&self, focus: &str, parts: &str) -> Vec<Part> {
        let focus = if focus.is_empty() { "main" } else { focus };
        let ws = self.dir_of(focus).and_then(|d| self.spawn_plugins.get(&d).map(|p| p.ws.clone())).unwrap_or_else(|| self.opts.paths.workspace.clone());
        crate::plugin_hint::of_json(parts, || {
            crate::plugin_hint::known_of(&bend_plugins::resolve::resolve(&bend_plugins::resolve::Roots::standard(Some(&ws))))
        })
    }
}

/// His words with the hub's hints after them: his parts (his words as one
/// text part when he sent none) then `hints`, and the model's text,
/// `render` of that one list, so the two agree. Pure.
fn hinted(said: &str, parts: &str, hints: Vec<Part>) -> (String, String) {
    let mut list: Vec<Part> = match serde_json::from_str::<Vec<Part>>(parts) {
        Ok(l) if !l.is_empty() => l,
        _ if said.is_empty() => Vec::new(),
        _ => vec![Part::Text { text: said.to_string() }],
    };
    list.extend(hints);
    let text = bise_proto::parts::render(&list);
    (text, serde_json::to_string(&list).unwrap_or_default())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// at-plugins: his message with a plugin part reaches the model as he
    /// typed it, the plugin's line after it; his bubble's words stay his.
    #[test]
    fn a_plugin_part_gets_its_line_after_his_words() {
        let his = serde_json::to_string(&vec![Part::Plugin { name: "computer".into() }, Part::Text { text: " open my last Gmail tab".into() }]).unwrap();
        let known = || vec![crate::plugin_hint::Known { name: "computer".into(), on: true, skills: vec!["computer:computer-use".into()], group: Some("computer".into()) }];
        let (text, parts) = hinted("ignored", &his, crate::plugin_hint::of_json(&his, known));
        assert_eq!(
            text,
            "@computer open my last Gmail tab\n\n[@computer: he picked the computer plugin for this turn: load its skill `computer:computer-use` with the skill tool first, then use its tools in tools.computer]"
        );
        let back: Vec<Part> = serde_json::from_str(&parts).unwrap();
        assert_eq!(bise_proto::parts::words(&back), "@computer open my last Gmail tab", "his words alone");
        // no plugin part: nothing resolved, nothing added
        let plain = serde_json::to_string(&vec![Part::Text { text: "@computer as words".into() }]).unwrap();
        assert!(crate::plugin_hint::of_json(&plain, || panic!("resolved for no plugin part")).is_empty());
        assert!(crate::plugin_hint::of_json("", || panic!("resolved for no parts")).is_empty());
    }

    /// The capsule's input carries the line main's rule reads (prompts.rs);
    /// any other client's input, and a slash command, stay as they are.
    #[test]
    fn an_input_from_the_capsule_carries_the_hint_main_reads() {
        let h = |text: &str, via: &str| Shell::capsule_hint(text, via).into_iter().collect::<Vec<_>>();
        // the model's text: today's bytes, byte for byte (the hint after his words)
        let (text, parts) = hinted("what's running?", "", h("what's running?", "capsule"));
        assert_eq!(text, "what's running?\n\n[from the capsule: answer in one short line]");
        let back: Vec<Part> = serde_json::from_str(&parts).unwrap();
        assert_eq!(bise_proto::parts::words(&back), "what's running?", "his words alone");
        assert!(bise_proto::parts::via_fn(&back, false));
        // main's rule names the same line (prompts.rs)
        assert!(crate::prompts::main_role("/w", "/t", "", true).contains("a `[from the capsule…]` line"));
        assert!(h("what's running?", "").is_empty() && h("hi", "tui").is_empty());
        assert!(h("/answer 3 1", "capsule").is_empty(), "never on a slash command");
        // fn space + tab: main starts an agent on it (ambient-lead m_6513)
        let (text, _) = hinted("start an agent for: fix the login loop", "", h("start an agent for: fix the login loop", "capsule-start"));
        assert_eq!(text, "start an agent for: fix the login loop\n\n[from the capsule: start an agent; answer in one short line]");
        // his own parts stay first; the steps line after the capsule's, as before
        let his = serde_json::to_string(&vec![Part::Text { text: "done with ".into() }, Part::Paste { n: 1, text: "x".into() }]).unwrap();
        let mut hs = h("done", "capsule");
        hs.extend(bise_proto::parts::steps_hint(&["p1 i2 \"pay\"".into()]));
        let (text, _) = hinted("ignored: his parts win", &his, hs);
        assert!(text.starts_with("done with <pasted n=\"1\""), "{text}");
        assert!(text.ends_with("\n\n[from the capsule: answer in one short line]\n[his open steps: p1 i2 \"pay\"; when he says one is done: sb page tick <page> <item>]"), "{text}");
    }
}
