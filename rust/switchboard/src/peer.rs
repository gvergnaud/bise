//! Who may do what on a hub's two sockets (docs/issues/16). Pure: the
//! socket, the first line's op and a process table in; a decision out.
//! The shell (`daemon/accept.rs`, `peer_os.rs`) reads the peer's pid and
//! the table, and only calls this.
//!
//! - `agent.sock` (the agents' `SB_SOCKET`): agent requests (`agent`,
//!   `version`) and `ping`, nothing else;
//! - `hub.sock` (clients: the TUI, the desktop's core, tests): `hello`
//!   only from a process that is not an agent's ([`judge`]; a peer whose
//!   process is gone is refused), `notice` unless it is proven an agent's;
//!   `ping`; and, until the release after v2026.10.2-25, agent requests
//!   from REPLs adopted from an older hub, whose `SB_SOCKET` is still
//!   hub.sock (a shim, logged per request).
//!
//! An agent's process: one whose parent chain, or the session of one of
//! them, holds a process tagged (`BISE_OWNERS`, `procs`) by this hub or
//! by a sibling hub (another project's, same `hubs/` folder), unless that
//! tag or process is the hub's own owner: a throwaway hub an agent started
//! (a test) lets that agent's processes be its clients. A double fork
//! that leaves the session and drops the variable escapes it: this is
//! defense in depth, the sandbox's deny of hub.sock is the wall.

use crate::procs::{parse_tag, Proc};
use std::collections::{BTreeMap, BTreeSet};

/// Which socket a connection came in on.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Sock {
    /// `agent.sock`: the agents' `sb`.
    Agent,
    /// `hub.sock`: the user's clients.
    Client,
}

/// What a socket does with a connection's first op.
#[derive(Debug, PartialEq, Eq)]
pub enum Access {
    /// Served to anyone (`ping`).
    Open,
    /// An agent request, served as today (`agent`, `version`).
    Agent,
    /// An agent request on hub.sock: served, with one hub.log line (REPLs
    /// adopted from an older hub; ends in the release after v2026.10.2-25).
    Shim,
    /// The user's: served only when [`judge`] says [`Who::Outside`].
    User,
    /// Not served on this socket.
    No,
}

/// What `sock` does with a first line whose op is `op`.
pub fn access(sock: Sock, op: &str) -> Access {
    match (sock, op) {
        (_, "ping") => Access::Open,
        (Sock::Agent, "agent" | "version") => Access::Agent,
        (Sock::Client, "agent" | "version") => Access::Shim,
        (Sock::Client, "hello" | "notice") => Access::User,
        _ => Access::No,
    }
}

/// What the hub knows of itself, for [`judge`].
#[derive(Clone, Debug, Default)]
pub struct HubSide {
    pub pid: u32,
    /// Its id in the tags (`procs::hub_id` of its socket).
    pub id: String,
    /// The tags it inherited (its own removed): the agents that started
    /// it, its owners.
    pub owners: BTreeSet<String>,
    /// The ids of the hubs next to it (its `hubs/` folder), its own too.
    pub siblings: BTreeSet<String>,
}

/// Who opened a connection.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Who {
    /// Not an agent's process: the user's.
    Outside,
    /// A process of this hub's agent (its folder name).
    Agent(String),
    /// A process of an agent of another project's hub.
    OtherHub(String),
    /// The process is gone or could not be read: refused (fail closed).
    Gone,
}

impl Who {
    /// The refusal of the first op `op`: a `hello` (the user's authority)
    /// is refused unless [`Who::Outside`] (fail closed); a `notice` (a
    /// line in main's thread, sent by a process that writes and closes at
    /// once) only when it is proven an agent's.
    pub fn refusal_of(&self, op: &str) -> Option<String> {
        match (op, self) {
            ("notice", Who::Gone) => None,
            _ => self.refusal(),
        }
    }

    /// The line the caller prints when refused; None when served.
    pub fn refusal(&self) -> Option<String> {
        match self {
            Who::Outside => None,
            Who::Agent(d) => Some(format!("this connection comes from agent {d}'s process: clients must be started by the user")),
            Who::OtherHub(d) => Some(format!(
                "this connection comes from agent {d}'s process (another project's hub): clients must be started by the user"
            )),
            Who::Gone => Some("the hub could not read which process opened this connection (gone already?): clients must be started by the user and stay connected".into()),
        }
    }
}

/// A process and its parents up to pid 1, or to a pid in `stop`.
fn chain(by_pid: &BTreeMap<u32, &Proc>, from: u32, stop: &BTreeSet<u32>) -> Vec<u32> {
    let mut out = vec![];
    let mut seen = BTreeSet::new();
    let mut cur = from;
    while cur > 1 && !stop.contains(&cur) && seen.insert(cur) {
        let Some(p) = by_pid.get(&cur) else { break };
        out.push(cur);
        cur = p.ppid;
    }
    out
}

/// Who `peer` is, from the process table `procs` (pure).
pub fn judge(procs: &[Proc], peer: u32, hub: &HubSide) -> Who {
    let by_pid: BTreeMap<u32, &Proc> = procs.iter().map(|p| (p.pid, p)).collect();
    if by_pid.get(&peer).is_none_or(|p| p.zombie) {
        return Who::Gone;
    }
    // the hub and its parents: the hub's own children (its switch, its
    // sb-core) stop there, and its starters are its owners
    let own: BTreeSet<u32> = chain(&by_pid, hub.pid, &BTreeSet::new()).into_iter().chain([hub.pid]).collect();
    let line = chain(&by_pid, peer, &own);
    let sessions = line.iter().filter_map(|p| by_pid.get(p)).map(|p| p.sid).filter(|s| *s > 1 && !own.contains(s));
    let mut seen = BTreeSet::new();
    for pid in line.iter().copied().chain(sessions) {
        if !seen.insert(pid) {
            continue;
        }
        let Some(owners) = by_pid.get(&pid).and_then(|p| p.owners.as_deref()) else { continue };
        for raw in owners.split(',').map(str::trim).filter(|t| !t.is_empty()) {
            let Some(t) = parse_tag(raw) else { continue };
            if hub.owners.contains(raw) {
                continue;
            }
            if t.hub == hub.id {
                return Who::Agent(t.dir);
            }
            if hub.siblings.contains(&t.hub) {
                return Who::OtherHub(t.dir);
            }
        }
    }
    Who::Outside
}

#[cfg(test)]
mod tests {
    use super::*;

    fn p(pid: u32, ppid: u32, sid: u32, owners: Option<&str>) -> Proc {
        Proc { pid, ppid, start: String::new(), zombie: false, sid, owners: owners.map(str::to_string), command: format!("/bin/p{pid}") }
    }

    /// hub `aa` (pid 100, under launchd, its own session), a sibling hub
    /// `bb`; agent t1's REPL 200 (own session), its bash 201, nc 202.
    fn world() -> (Vec<Proc>, HubSide) {
        let procs = vec![
            p(1, 0, 1, None),
            p(100, 1, 100, Some("")),
            p(200, 100, 200, Some("aa.t1.5")),
            p(201, 200, 200, Some("aa.t1.5")),
            p(202, 201, 200, None),
            // the user's terminal and TUI
            p(300, 1, 300, None),
            p(301, 300, 300, None),
            // project bb's hub and its agent x's REPL
            p(400, 1, 400, Some("")),
            p(401, 400, 401, Some("bb.x.9")),
            p(402, 401, 401, None),
        ];
        let hub = HubSide { pid: 100, id: "aa".into(), owners: BTreeSet::new(), siblings: ["aa".to_string(), "bb".to_string()].into() };
        (procs, hub)
    }

    #[test]
    fn the_sockets_serve_their_ops() {
        assert_eq!(access(Sock::Agent, "agent"), Access::Agent);
        assert_eq!(access(Sock::Agent, "version"), Access::Agent);
        assert_eq!(access(Sock::Agent, "ping"), Access::Open);
        for op in ["hello", "notice", "stop_hub", ""] {
            assert_eq!(access(Sock::Agent, op), Access::No, "{op}");
        }
        assert_eq!(access(Sock::Client, "hello"), Access::User);
        assert_eq!(access(Sock::Client, "notice"), Access::User);
        assert_eq!(access(Sock::Client, "ping"), Access::Open);
        assert_eq!(access(Sock::Client, "agent"), Access::Shim);
        assert_eq!(access(Sock::Client, "version"), Access::Shim);
        assert_eq!(access(Sock::Client, "whatever"), Access::No);
    }

    #[test]
    fn the_users_client_is_outside() {
        let (procs, hub) = world();
        assert_eq!(judge(&procs, 301, &hub), Who::Outside);
    }

    #[test]
    fn a_process_below_this_hubs_repl_is_its_agent() {
        let (procs, hub) = world();
        // nc hides its environment (an Apple binary): its parents tell
        assert_eq!(judge(&procs, 202, &hub), Who::Agent("t1".into()));
        assert_eq!(judge(&procs, 200, &hub), Who::Agent("t1".into()));
    }

    #[test]
    fn a_double_fork_in_the_repls_session_is_still_its_agent() {
        let (mut procs, hub) = world();
        procs.push(p(203, 1, 200, None));
        assert_eq!(judge(&procs, 203, &hub), Who::Agent("t1".into()));
    }

    #[test]
    fn a_tagged_process_reparented_out_of_the_session_is_its_agent() {
        let (mut procs, hub) = world();
        procs.push(p(204, 1, 204, Some("aa.t1.5")));
        assert_eq!(judge(&procs, 204, &hub), Who::Agent("t1".into()));
    }

    #[test]
    fn an_agent_of_another_projects_hub_is_refused() {
        let (procs, hub) = world();
        assert_eq!(judge(&procs, 402, &hub), Who::OtherHub("x".into()));
    }

    #[test]
    fn an_agent_of_a_hub_elsewhere_is_outside() {
        // a hub in another home (a throwaway hub's outer world)
        let (mut procs, hub) = world();
        procs.push(p(500, 1, 500, Some("cc.y.1")));
        procs.push(p(501, 500, 500, None));
        assert_eq!(judge(&procs, 501, &hub), Who::Outside);
    }

    #[test]
    fn a_throwaway_hub_serves_the_agent_that_started_it() {
        // hub tt (pid 600) started by a test (601) under agent y's REPL
        // (500) of hub aa: y's test client (602) may say hello to tt
        let (mut procs, _) = world();
        procs.push(p(500, 100, 500, Some("aa.y.1")));
        procs.push(p(601, 500, 500, None));
        procs.push(p(600, 601, 500, None));
        procs.push(p(602, 601, 500, None));
        // tt's own agent
        procs.push(p(610, 600, 610, Some("tt.z.2")));
        procs.push(p(611, 610, 610, None));
        let tt = HubSide { pid: 600, id: "tt".into(), owners: BTreeSet::new(), siblings: ["tt".to_string()].into() };
        assert_eq!(judge(&procs, 602, &tt), Who::Outside);
        assert_eq!(judge(&procs, 611, &tt), Who::Agent("z".into()));
    }

    #[test]
    fn the_hubs_owner_tags_are_not_agents() {
        // a hub that inherited aa.y.1 (started below y, then reparented):
        // y's processes are its owner's, not its agents'
        let (mut procs, _) = world();
        procs.push(p(700, 1, 700, Some("aa.y.1")));
        procs.push(p(701, 700, 700, None));
        let me = HubSide { pid: 650, id: "aa".into(), owners: ["aa.y.1".to_string()].into(), siblings: ["aa".to_string()].into() };
        procs.push(p(650, 1, 650, Some("aa.y.1")));
        assert_eq!(judge(&procs, 701, &me), Who::Outside);
    }

    #[test]
    fn the_hubs_own_children_are_outside() {
        // a hub whose session leader is an agent's REPL (no setsid): its
        // switch process shares that session and stops at the hub
        let (mut procs, mut hub) = world();
        procs.push(p(800, 200, 200, Some("zz.q.1")));
        procs.push(p(801, 800, 200, None));
        hub.pid = 800;
        hub.id = "zz".into();
        hub.siblings = ["zz".to_string()].into();
        assert_eq!(judge(&procs, 801, &hub), Who::Outside);
    }

    #[test]
    fn a_gone_or_zombie_peer_is_refused() {
        let (mut procs, hub) = world();
        assert_eq!(judge(&procs, 999, &hub), Who::Gone);
        procs.push(Proc { zombie: true, ..p(205, 201, 200, None) });
        assert_eq!(judge(&procs, 205, &hub), Who::Gone);
        assert!(Who::Gone.refusal().is_some() && Who::Outside.refusal().is_none());
    }

    #[test]
    fn launchd_at_the_top_ends_the_walk() {
        // a loop in a corrupt table ends too
        let (mut procs, hub) = world();
        procs.push(p(900, 901, 900, None));
        procs.push(p(901, 900, 900, None));
        assert_eq!(judge(&procs, 900, &hub), Who::Outside);
        assert_eq!(judge(&procs, 301, &hub), Who::Outside);
    }

    #[test]
    fn a_hello_fails_closed_but_a_notice_only_on_proof() {
        // a notice is written, then the socket closed: its pid may be gone
        assert!(Who::Gone.refusal_of("hello").is_some());
        assert!(Who::Gone.refusal_of("notice").is_none());
        assert!(Who::Agent("t1".into()).refusal_of("notice").is_some());
        assert!(Who::OtherHub("x".into()).refusal_of("notice").is_some());
        assert!(Who::Outside.refusal_of("hello").is_none());
    }

    #[test]
    fn the_refusal_names_the_agent() {
        assert!(Who::Agent("t1".into()).refusal().unwrap().contains("agent t1's process"));
    }
}
