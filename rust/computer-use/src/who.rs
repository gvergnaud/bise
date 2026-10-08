//! Who is at the other end of a broker connection (docs/issues/18), pure
//! but for [`real`]: what each kind of connection may do, and an agent's
//! key.
//!
//! - the command socket (`computer-use-ctl.sock`): the user's only
//!   ([`Peer::Outside`]); an agent's process, or one the broker can't
//!   read, is refused with one line;
//! - the agents' socket (`computer-use.sock`): a browser relay must be
//!   outside too (Chrome starts it); an agent's MCP server is keyed by
//!   the tag of its process, never by the name in its hello.
//!
//! An agent's key: `<hub id>.<dir>` for a process of a hub's agent (the
//! last tag of the nearest tagged process, `bise_peer::judge::judge_any`),
//! the hello's name for an untagged one (`bise --headless`, a user's
//! script). Its name (what the user sees) is the dir, or that name.

pub use bise_peer::judge::Peer;
use std::os::unix::net::UnixStream;

/// The line a refused command connection gets.
pub const CTL_REFUSED: &str = "computer use's commands are the user's: an agent's process can't run them";

/// What reads a connection's peer: the real one in the commands, a fake in
/// tests (a test runs inside an agent, so every real peer is an agent's).
pub type Judge = dyn Fn(&UnixStream) -> Peer + Send + Sync;

/// The broker's [`Judge`] (in its `Opts`).
#[derive(Clone)]
pub struct JudgeFn(pub std::sync::Arc<Judge>);

impl JudgeFn {
    pub fn real() -> JudgeFn {
        JudgeFn(std::sync::Arc::new(real))
    }
}

impl std::fmt::Debug for JudgeFn {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("JudgeFn")
    }
}

/// The peer from its pid and the process table (`me`: the broker).
pub fn real(s: &UnixStream) -> Peer {
    match bise_peer::os::peer_pid(s) {
        Some(pid) => bise_peer::judge::judge_any(&bise_peer::table::snapshot(), pid, std::process::id()),
        None => Peer::Gone,
    }
}

/// A command connection: None when served, else the refusal.
pub fn ctl_refusal(peer: &Peer) -> Option<&'static str> {
    match peer {
        Peer::Outside => None,
        _ => Some(CTL_REFUSED),
    }
}

/// An agent connection's key, or None (refused: the peer is gone).
pub fn agent_key(peer: &Peer, hello_name: &str) -> Option<String> {
    match peer {
        Peer::Agent(t) => Some(bise_peer::tags::agent_key(&t.hub, &t.dir)),
        Peer::Outside if !hello_name.is_empty() => Some(hello_name.to_string()),
        _ => None,
    }
}

/// A key's hub id (None: an untagged agent) and its name.
pub fn split(key: &str) -> (Option<&str>, &str) {
    match key.split_once('.') {
        Some((hub, dir)) if hub.len() == 16 && hub.bytes().all(|b| b.is_ascii_hexdigit()) && !dir.is_empty() => (Some(hub), dir),
        _ => (None, key),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use bise_peer::tags::parse_tag;

    const HUB: &str = "00000000000000aa";

    #[test]
    fn commands_are_the_users_only() {
        assert_eq!(ctl_refusal(&Peer::Outside), None);
        assert_eq!(ctl_refusal(&Peer::Gone), Some(CTL_REFUSED));
        assert_eq!(ctl_refusal(&Peer::Agent(parse_tag(&format!("{HUB}.perf.1")).unwrap())), Some(CTL_REFUSED));
    }

    #[test]
    fn an_agent_is_keyed_by_its_tag_never_its_hello() {
        let tag = Peer::Agent(parse_tag(&format!("{HUB}.perf.1")).unwrap());
        assert_eq!(agent_key(&tag, "main").as_deref(), Some("00000000000000aa.perf"));
        assert_eq!(agent_key(&Peer::Outside, "bench").as_deref(), Some("bench"));
        assert_eq!(agent_key(&Peer::Outside, ""), None);
        assert_eq!(agent_key(&Peer::Gone, "perf"), None);
    }

    #[test]
    fn a_key_splits_into_hub_and_name() {
        assert_eq!(split("00000000000000aa.perf"), (Some(HUB), "perf"));
        assert_eq!(split("00000000000000aa.t_1.b"), (Some(HUB), "t_1.b"));
        assert_eq!(split("bench"), (None, "bench"));
        assert_eq!(split("my.agent"), (None, "my.agent"));
    }
}
