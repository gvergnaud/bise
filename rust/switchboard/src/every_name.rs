//! The names of `sb every`'s timers (sched-names, architect m_14532): a
//! timer set without `--name` gets 2-5 words from the small-jobs model,
//! asked off the hub's loop like the role lines (role.rs), one call at a
//! time; no model or a failed call: bise_proto's plain fallback
//! (`words::timer_fallback`) at once, so a name is always kept.
//!
//! Pure. The name itself is sb-core's state (hub/timers.bend: `name`,
//! the `every_name` journal line); here, which timer to ask about next
//! ([`Asker::next`]), the request and the reply's [`clean`]ing. When the
//! ◷ set line goes is sb-core's (a timer set without a name is `held`,
//! its every_name `announce`s it: architect m_15603). core.rs only calls
//! these; the daemon runs the call (daemon/small_ask.rs).

use crate::every::Timers;
use std::collections::BTreeSet;

/// The mark the system prompt starts with (the fake provider of the tests
/// answers it with a fixed name).
pub const MARK: &str = "# bise timer name";

const SYSTEM: &str = "# bise timer name\nYou name a scheduled task in a list of an AI agents app. \
The task is a message sent to an agent at a fixed time; its name says the job in 2 to 5 words. \
Rules: lowercase, a verb or a noun phrase, plain words, no paths, no ids, no quotes, no question mark, \
no final period, at most 32 characters. Name the job; never quote the message's first line. \
The agent's name is shown beside it: never put it in the name. \
Examples: check the nightly build / streaming bench / desktop drive review. \
Answer with the name only.";

/// The calls in flight and done. Runtime only (a restarted hub asks
/// again for a timer still unnamed).
#[derive(Debug, Default)]
pub struct Asker {
    /// the timer whose call is in flight
    asking: Option<u64>,
    /// timers already asked about (answered or failed): never twice
    asked: BTreeSet<u64>,
}

impl Asker {
    /// The next call, if none is in flight: the first live timer without
    /// a name not asked about yet; its id and the wire request.
    pub fn next(&mut self, timers: &Timers) -> Option<(u64, String)> {
        if self.asking.is_some() {
            return None;
        }
        let t = timers.map.values().find(|t| needs_name(&t.name) && !self.asked.contains(&t.id))?;
        self.asking = Some(t.id);
        self.asked.insert(t.id);
        Some((t.id, request(&t.text, &t.agent)))
    }

    /// The call of `id` ended.
    pub fn done(&mut self, id: u64) {
        if self.asking == Some(id) {
            self.asking = None;
        }
    }
}

/// Whether this journal line writes the timer's ◷ set line now. sb-core
/// decides it (architect m_15603): a timer set without a name is `held`
/// and its every_name says `announce`, once, whatever restarts come
/// between (law named_announces_once); the line carries its name
/// (designer m_14531). An older hub's every_set (no `held`) goes at once.
pub fn set_line_now(ev: &serde_json::Value) -> bool {
    match ev["type"].as_str() {
        Some("every_set") => ev["held"] != true,
        Some("every_name") => ev["announce"] == true,
        _ => false,
    }
}

/// The wire request (runtime/remote.bend's format, like role.rs) of one
/// call: the instruction, one line, clipped.
pub fn request(text: &str, agent: &str) -> String {
    let user = format!(
        "The agent (shown beside the name): {}\nThe message: {}",
        agent,
        crate::util::clip(&crate::util::one_line(text), 1200)
    );
    format!("MODEL default\nMSG system : {}\nMSG user : {}\nEND\n", escape(SYSTEM), escape(&user))
}

fn escape(s: &str) -> String {
    s.replace('\\', "/").replace('\n', "\\n")
}

/// The name in a model's reply: its first line, without quotes, a `name:`
/// label or a final period, lowercase, at most 6 words, cut at a word to
/// 32 characters. None: nothing usable (a path, empty, a sentence): the
/// fallback names it.
pub fn clean(reply: &str) -> Option<String> {
    let reply = &without_think(reply);
    let first = reply.lines().map(str::trim).find(|l| !l.is_empty() && !l.starts_with("```"))?;
    let mut l = first.trim_start_matches(['-', '*', '#', '>', ' ']).trim().to_string();
    if l.to_lowercase().starts_with("name:") {
        l = l[5..].trim().to_string();
    }
    let l = l.trim_matches(|c: char| matches!(c, '"' | '\'' | '`' | '«' | '»' | '“' | '”' | '*' | '_')).trim();
    // a label (designer m_15602): 2-5 words, no path, no question
    if l.contains('?') {
        return None;
    }
    let l = l.trim_end_matches(['.', '!']).trim().to_lowercase();
    let words = l.split_whitespace().count();
    if words == 0 || words > 5 || l.contains(['/', '…', '<', '>']) || l.chars().any(|c| c.is_control()) {
        return None;
    }
    Some(bise_proto::thread::words::name_fit(&l))
}

/// The reply without the model's reasoning: every `<think>…</think>`
/// block, and an unclosed `<think>` with all that follows it (timer #162
/// was named '<think>').
fn without_think(reply: &str) -> String {
    let mut out = String::new();
    let mut rest = reply;
    while let Some(i) = rest.find("<think>") {
        out.push_str(&rest[..i]);
        match rest[i..].find("</think>") {
            Some(j) => rest = &rest[i + j + "</think>".len()..],
            None => return out,
        }
    }
    out.push_str(rest);
    out
}

/// A name the hub names again (like an unnamed one): empty, or a model's
/// tag kept by an older hub ('<think>').
pub fn needs_name(name: &str) -> bool {
    name.is_empty() || name.starts_with('<')
}

/// The name a call's end gives: the model's, else the plain fallback;
/// never with its agent's name in front (the row shows it).
pub fn name_of(reply: Option<&str>, text: &str, agent: &str) -> String {
    use bise_proto::thread::words::{name_without_agent, timer_fallback};
    match reply.and_then(clean) {
        Some(n) => name_without_agent(&n, agent),
        None => timer_fallback(text, agent),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::every::{Sched, Timer};

    fn timer(id: u64, name: &str) -> Timer {
        Timer {
            id,
            agent: "perf".into(),
            by: "main".into(),
            text: format!("check build {id}"),
            sched: Sched::Every(60_000),
            next_ms: 0,
            until_ms: None,
            times: None,
            fired: 0,
            page: None,
            last_ms: 0,
            runs: vec![],
            name: name.into(),
        }
    }

    /// One call at a time, each unnamed timer once, a named one never.
    #[test]
    fn one_call_at_a_time_and_once_per_timer() {
        let mut ts = Timers::default();
        for t in [timer(1, ""), timer(2, "named"), timer(3, "")] {
            ts.map.insert(t.id, t);
        }
        let mut a = Asker::default();
        let (id, req) = a.next(&ts).unwrap();
        assert_eq!(id, 1);
        assert!(req.contains(MARK) && req.contains("check build 1") && req.contains("The agent (shown beside the name): perf"), "{req}");
        assert!(a.next(&ts).is_none(), "one in flight");
        a.done(1);
        assert_eq!(a.next(&ts).map(|x| x.0), Some(3), "2 has a name");
        a.done(3);
        assert!(a.next(&ts).is_none(), "each once, even unnamed still");
        // a '<think>' name from an older hub is named again
        ts.map.insert(4, timer(4, "<think>"));
        assert_eq!(a.next(&ts).map(|x| x.0), Some(4));
    }

    #[test]
    fn the_reply_is_cleaned() {
        assert_eq!(clean("\"Streaming Bench.\"\n").as_deref(), Some("streaming bench"));
        assert_eq!(clean("Name: check the nightly build").as_deref(), Some("check the nightly build"));
        assert_eq!(clean("read /tmp/x.out"), None);
        assert_eq!(clean(""), None);
        assert_eq!(clean("this is a whole sentence about what the task does"), None);
        assert_eq!(clean("is the queue empty?"), None, "a label, not the question");
        assert_eq!(clean("Streaming Bench!").as_deref(), Some("streaming bench"));
        // the model's reasoning tag (timer #162): dropped, else the fallback
        assert_eq!(clean("<think>the user wants a name</think>\nstreaming bench").as_deref(), Some("streaming bench"));
        assert_eq!(clean("<think>"), None);
        assert_eq!(clean("<think>\nhmm, a name for this"), None);
        assert_eq!(clean("<b>bench</b>"), None);
        assert_eq!(name_of(Some("<think>"), "check the nightly build", "perf"), "check the nightly build");
        assert_eq!(name_of(None, "amb-core: streaming bench: queue empty? (pgrep", "amb-core"), "streaming bench");
        assert_eq!(name_of(Some("desktop drive review"), "x", "main"), "desktop drive review");
        assert_eq!(name_of(Some("amb-core streaming bench"), "x", "amb-core"), "streaming bench", "never its agent's name");
    }
}
