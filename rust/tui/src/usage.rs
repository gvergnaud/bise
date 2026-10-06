//! Token usage and context-window fill of an agent.
//!
//! After each model call the REPL prints one feed line
//! (runtime/usage-pure.bend):
//! `  obs: usage: model=M in=I out=O cache_read=R cache_write=W`
//! (parsed by bise_session::usage_line; the display is here).
//! `in` counts every input token of the call (cached ones included):
//! the context the model saw. The context after the call is `in + out`
//! (the reply joins the history). A compaction resets it: the last
//! usage before a `compaction_done` no longer describes the context.
//! `model` is the full `provider/model` id: its window and prices come
//! from bise's catalog (models.rs, BISE-150).

use crate::models::context_window;
use crate::Ev;

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Usage {
    pub model: String,
    pub input: u64,
    pub output: u64,
    pub cache_read: u64,
    pub cache_write: u64,
}

impl Usage {
    /// Parse the text after `obs: usage: `: the one parser of the line
    /// is bise_session::usage_line (the hub reads it too).
    pub fn parse(t: &str) -> Option<Usage> {
        bise_session::usage_line::parse(t).map(|u| Usage {
            model: u.model,
            input: u.input,
            output: u.output,
            cache_read: u.cache_read,
            cache_write: u.cache_write,
        })
    }

    /// Tokens in the context after the call.
    pub fn context(&self) -> u64 {
        self.input + self.output
    }

    /// "42k / 200k tokens · 21%"; a model with no known window: "42k tokens".
    pub fn label(&self) -> String {
        let used = self.context();
        match context_window(&self.model) {
            Some(w) => format!("{} / {} tokens · {}%", fmt_tokens(used), fmt_tokens(w), percent(used, w)),
            None => format!("{} tokens", fmt_tokens(used)),
        }
    }

    /// The divider's form at rest (BISE-303): "42k · 21%", or "42k"
    /// without a known window.
    pub fn compact(&self) -> String {
        let used = self.context();
        match context_window(&self.model) {
            Some(w) => format!("{} · {}%", fmt_tokens(used), percent(used, w)),
            None => fmt_tokens(used),
        }
    }

    /// The compact form for the task list: "21%", or "42k" without a
    /// known window.
    pub fn short(&self) -> String {
        let used = self.context();
        match context_window(&self.model) {
            Some(w) => format!("{}%", percent(used, w)),
            None => fmt_tokens(used),
        }
    }

    /// What the call cost in USD; None when the model's prices are not
    /// in the catalog.
    pub fn cost(&self) -> Option<f64> {
        crate::models::cost(&self.model, self.input, self.output, self.cache_read, self.cache_write)
    }

    /// The feed's usage line: `usage: 42k / 200k tokens · 21% (in 42000
    /// · out 500 · $0.0132)`, the cost when it is known.
    pub fn line(&self) -> String {
        let cost = self.cost().map(|c| format!(" · {}", crate::models::fmt_cost(c))).unwrap_or_default();
        format!("  usage: {} (in {} · out {}{})", self.label(), self.input, self.output, cost)
    }
}

fn percent(used: u64, window: u64) -> u64 {
    if window == 0 {
        return 0;
    }
    // saturating: a corrupt token count never overflows (debug panics)
    used.saturating_mul(100).saturating_add(window / 2) / window
}

/// 950 -> "950", 42_310 -> "42k", 1_250_000 -> "1.2M".
pub fn fmt_tokens(n: u64) -> String {
    if n < 1000 {
        n.to_string()
    } else if n < 1_000_000 {
        format!("{}k", (n + 500) / 1000)
    } else {
        let tenths = (n + 50_000) / 100_000;
        if tenths.is_multiple_of(10) {
            format!("{}M", tenths / 10)
        } else {
            format!("{}.{}M", tenths / 10, tenths % 10)
        }
    }
}

/// The usage of the current context of a feed: the last usage event,
/// unless a compaction came after it.
pub fn current(events: &[Ev]) -> Option<&Usage> {
    for e in events.iter().rev() {
        match e {
            Ev::Usage(u) => return Some(u),
            Ev::Compacted { .. } => return None,
            _ => {}
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_parsed_line_gives_the_context() {
        // the parse itself is tested in bise_session::usage_line
        let u = Usage::parse("model=claude-opus-5-5 in=40312 out=512 cache_read=40000 cache_write=300").unwrap();
        assert_eq!((u.input, u.output, u.cache_read, u.cache_write), (40312, 512, 40000, 300));
        assert_eq!(u.context(), 40824);
    }

    #[test]
    fn labels() {
        let u = Usage { model: "foundry/claude-opus-5-5".into(), input: 209_500, output: 500, ..Default::default() };
        assert_eq!(u.label(), "210k / 1M tokens · 21%");
        assert_eq!(u.short(), "21%");
        assert_eq!(u.compact(), "210k · 21%");
        // the window is the model's, not a guess from its name
        let s = Usage { model: "anthropic/claude-haiku-4-5".into(), input: 42_000, ..Default::default() };
        assert_eq!(s.label(), "42k / 200k tokens · 21%");
        let g = Usage { model: "mistral/zai-glm-5-3".into(), input: 42_000, output: 0, ..Default::default() };
        assert_eq!(g.label(), "42k / 1M tokens · 4%");
        // an old bare id: the legacy rule
        let o = Usage { model: "claude-opus-5-5".into(), input: 209_500, output: 500, ..Default::default() };
        assert_eq!(o.short(), "21%");
        // an unlisted model: its provider's default
        let x = Usage { model: "groq/some-model".into(), input: 13_107, output: 0, ..Default::default() };
        assert_eq!(x.label(), "13k / 131k tokens · 10%");
        let n = Usage { model: "nowhere/some-model".into(), input: 950, output: 0, ..Default::default() };
        assert_eq!(n.label(), "950 tokens");
        assert_eq!(n.short(), "950");
        assert_eq!(n.compact(), "950");
    }

    #[test]
    fn the_usage_line_shows_the_cost_when_prices_are_known() {
        let u = Usage { model: "anthropic/claude-haiku-4-5".into(), input: 10_000, output: 1_000, ..Default::default() };
        assert_eq!(u.line(), "  usage: 11k / 200k tokens · 6% (in 10000 · out 1000 · $0.0150)");
        // no price listed: no cost
        let f = Usage { model: "fireworks/accounts/fireworks/models/glm-5p3".into(), input: 10_000, output: 1_000, ..Default::default() };
        assert_eq!(f.line(), "  usage: 11k / 1M tokens · 1% (in 10000 · out 1000)");
    }

    #[test]
    fn formats_tokens() {
        assert_eq!(fmt_tokens(0), "0");
        assert_eq!(fmt_tokens(999), "999");
        assert_eq!(fmt_tokens(42_310), "42k");
        assert_eq!(fmt_tokens(1_000_000), "1M");
        assert_eq!(fmt_tokens(1_250_000), "1.3M");
    }

    #[test]
    fn the_feed_line_is_a_hidden_event() {
        let ev = crate::parse_line("  obs: usage: model=claude-opus-5-5 in=100 out=5 cache_read=0 cache_write=0");
        assert!(matches!(&ev, Some(Ev::Usage(u)) if u.input == 100 && u.output == 5));
        assert!(!crate::ev_visible(&ev.unwrap(), false));
    }

    #[test]
    fn compaction_resets_the_count() {
        let u = |n| Ev::Usage(Usage { model: "claude-x".into(), input: n, ..Default::default() });
        let evs = vec![u(10), Ev::Info("x".into()), u(20)];
        assert_eq!(current(&evs).map(|u| u.input), Some(20));
        let evs = vec![u(10), u(900), Ev::Compacted { text: "sum...".into(), open: false }];
        assert_eq!(current(&evs), None);
        let evs = vec![u(900), Ev::Compacted { text: "sum...".into(), open: false }, u(30)];
        assert_eq!(current(&evs).map(|u| u.input), Some(30));
        assert_eq!(current(&[]), None);
    }
}
