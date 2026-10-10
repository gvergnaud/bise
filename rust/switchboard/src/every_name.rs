//! The names of `sb every`'s timers (sched-names, architect m_14532): a
//! timer set without `--name` gets 2-5 words from the small-jobs model,
//! asked off the hub's loop like the role lines (role.rs), one call at a
//! time; no model or a failed call: bise_proto's plain fallback
//! (`words::timer_fallback`) at once, so a name is always kept.
//!
//! Pure. The name itself is sb-core's state (hub/timers.bend: `name`,
//! the `every_name` journal line); here, which timer to ask about next
//! ([`Asker::next`]), the request, the reply's [`clean`]ing, and the set
//! line a fresh timer waits to write until it has its name. core.rs only
//! calls these; the daemon runs the call (daemon/small_ask.rs).

use crate::every::Timers;
use std::collections::BTreeSet;

/// The mark the system prompt starts with (the fake provider of the tests
/// answers it with a fixed name).
pub const MARK: &str = "# bise timer name";

const SYSTEM: &str = "# bise timer name\nYou name a scheduled task in a list of an AI agents app. \
The task is a message sent to an agent at a fixed time; its name says the job in 2 to 5 words. \
Rules: lowercase, a verb or a noun phrase, plain words, no paths, no ids, no quotes, no final period, \
at most 32 characters. Examples: check the nightly build / streaming bench / desktop drive review. \
Answer with the name only.";

/// The calls in flight and done, and the set lines that wait for a name.
/// Runtime only (a restarted hub asks again for a timer still unnamed).
#[derive(Debug, Default)]
pub struct Asker {
    /// the timer whose call is in flight
    asking: Option<u64>,
    /// timers already asked about (answered or failed): never twice
    asked: BTreeSet<u64>,
    /// timers set in this run without a name: their ◷ set line waits for it
    fresh: BTreeSet<u64>,
}

impl Asker {
    /// The next call, if none is in flight: the first live timer without
    /// a name not asked about yet; its id and the wire request.
    pub fn next(&mut self, timers: &Timers) -> Option<(u64, String)> {
        if self.asking.is_some() {
            return None;
        }
        let t = timers.map.values().find(|t| t.name.is_empty() && !self.asked.contains(&t.id))?;
        self.asking = Some(t.id);
        self.asked.insert(t.id);
        Some((t.id, request(&t.text)))
    }

    /// The call of `id` ended.
    pub fn done(&mut self, id: u64) {
        if self.asking == Some(id) {
            self.asking = None;
        }
    }

    /// A timer set now without a name: its set line waits.
    pub fn hold(&mut self, id: u64) {
        self.fresh.insert(id);
    }

    /// A timer got its name: whether its set line was waiting for it.
    pub fn release(&mut self, id: u64) -> bool {
        self.fresh.remove(&id)
    }
}

/// The wire request (runtime/remote.bend's format, like role.rs) of one
/// call: the instruction, one line, clipped.
pub fn request(text: &str) -> String {
    let user = format!("The message: {}", crate::util::clip(&crate::util::one_line(text), 1200));
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
    let first = reply.lines().map(str::trim).find(|l| !l.is_empty() && !l.starts_with("```"))?;
    let mut l = first.trim_start_matches(['-', '*', '#', '>', ' ']).trim().to_string();
    if l.to_lowercase().starts_with("name:") {
        l = l[5..].trim().to_string();
    }
    let l = l
        .trim_matches(|c: char| matches!(c, '"' | '\'' | '`' | '«' | '»' | '“' | '”' | '*' | '_'))
        .trim()
        .trim_end_matches('.')
        .trim()
        .to_lowercase();
    let words = l.split_whitespace().count();
    if words == 0 || words > 6 || l.contains('/') || l.chars().any(|c| c.is_control()) {
        return None;
    }
    Some(bise_proto::thread::words::name_fit(&l))
}

/// The name a call's end gives: the model's, else the plain fallback.
pub fn name_of(reply: Option<&str>, text: &str) -> String {
    reply.and_then(clean).unwrap_or_else(|| bise_proto::thread::words::timer_fallback(text))
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
        assert!(req.contains(MARK) && req.contains("check build 1"), "{req}");
        assert!(a.next(&ts).is_none(), "one in flight");
        a.done(1);
        assert_eq!(a.next(&ts).map(|x| x.0), Some(3), "2 has a name");
        a.done(3);
        assert!(a.next(&ts).is_none(), "each once, even unnamed still");
    }

    #[test]
    fn a_held_set_line_is_released_once() {
        let mut a = Asker::default();
        a.hold(4);
        assert!(a.release(4));
        assert!(!a.release(4));
        assert!(!a.release(5));
    }

    #[test]
    fn the_reply_is_cleaned() {
        assert_eq!(clean("\"Streaming Bench.\"\n").as_deref(), Some("streaming bench"));
        assert_eq!(clean("Name: check the nightly build").as_deref(), Some("check the nightly build"));
        assert_eq!(clean("read /tmp/x.out"), None);
        assert_eq!(clean(""), None);
        assert_eq!(clean("this is a whole sentence about what the task does"), None);
        assert_eq!(name_of(None, "amb-core: read $TMPDIR/s4.log (S29 bench)"), "read S29 bench");
        assert_eq!(name_of(Some("desktop drive review"), "x"), "desktop drive review");
    }
}
