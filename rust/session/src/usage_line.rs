//! The REPL's usage line, the one parser of it: after each model call
//! the REPL prints (bend/runtime/usage-pure.bend)
//! `  obs: usage: model=M in=I out=O cache_read=R cache_write=W`.
//! `in` counts every input token of the call, cached ones included
//! (bend/core/api.bend usage_of folds them in for every family): the
//! context the model saw. The TUI shows the context fill from it
//! (rust/tui/src/usage.rs); the hub counts what an agent's REPL has
//! churned through since it started (rust/switchboard/src/recycle.rs).

/// One usage line's numbers.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct UsageLine {
    pub model: String,
    pub input: u64,
    pub output: u64,
    pub cache_read: u64,
    pub cache_write: u64,
}

/// The prefix of the line on the wire, before [`parse`]'s text.
pub const PREFIX: &str = "  obs: usage: ";

/// Parse the text after `obs: usage: `. Unknown keys are ignored; a
/// text without `in=` is not a usage line.
pub fn parse(t: &str) -> Option<UsageLine> {
    let mut u = UsageLine::default();
    let mut has_in = false;
    for kv in t.split_whitespace() {
        let Some((k, v)) = kv.split_once('=') else { continue };
        let n = || v.parse::<u64>().ok();
        match k {
            "model" => u.model = v.to_string(),
            "in" => {
                u.input = n()?;
                has_in = true;
            }
            "out" => u.output = n()?,
            "cache_read" => u.cache_read = n()?,
            "cache_write" => u.cache_write = n()?,
            _ => {}
        }
    }
    has_in.then_some(u)
}

/// A whole wire line (`  obs: usage: ...`); None for any other line.
pub fn of_line(line: &str) -> Option<UsageLine> {
    parse(line.strip_prefix(PREFIX)?)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_the_repl_line() {
        let u = parse("model=claude-opus-5-5 in=40312 out=512 cache_read=40000 cache_write=300").unwrap();
        assert_eq!(u.model, "claude-opus-5-5");
        assert_eq!((u.input, u.output, u.cache_read, u.cache_write), (40312, 512, 40000, 300));
        assert!(parse("model=m out=3").is_none());
        assert!(parse("model=m in=x").is_none());
    }

    #[test]
    fn a_wire_line_needs_its_prefix() {
        let u = of_line("  obs: usage: model=m in=7 out=1 cache_read=0 cache_write=0").unwrap();
        assert_eq!((u.input, u.output), (7, 1));
        assert!(of_line("  obs: turn_done: completed").is_none());
        assert!(of_line("model=m in=7").is_none());
        assert!(of_line("you : obs: usage: model=m in=7").is_none());
    }
}
