//! Who opened a connection, pure: a peer pid and the process table in, a
//! decision out (docs/issues/16, 18). The callers read the pid
//! ([`crate::os::peer_pid`]) and the table ([`crate::table::snapshot`]),
//! and only call this.
//!
//! An agent's process: one whose parent chain, or the session of one of
//! them, holds a process tagged (`BISE_OWNERS`, [`crate::tags`]). macOS
//! hides the environment of its own binaries, so a process with no
//! visible list in a REPL's session (each REPL runs `setsid`) is that
//! REPL's agent's. A double fork that leaves the session and drops the
//! variable escapes the walk: this is defense in depth, a sandbox deny of
//! the socket is the wall.
//!
//! - [`judge`]: for one hub (`hub.sock`): this hub's agent, another
//!   project's (a sibling hub), or outside; the hub's own owners (the
//!   agent that started a throwaway hub) are not its agents.
//! - [`judge_any`]: for a machine-wide service (the computer-use broker):
//!   any hub's agent is an agent, keyed by its tag.

use crate::table::Proc;
use crate::tags::{parse_tag, Tag};
use std::collections::{BTreeMap, BTreeSet};

/// What the hub knows of itself, for [`judge`].
#[derive(Clone, Debug, Default)]
pub struct HubSide {
    pub pid: u32,
    /// Its id in the tags (`tags::hub_id` of its socket).
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

/// The processes whose tags speak for `peer`, nearest first: its chain
/// (stopped at `stop`), then the sessions of that chain (minus
/// `skip_sessions`), each once.
fn walk(by_pid: &BTreeMap<u32, &Proc>, peer: u32, stop: &BTreeSet<u32>, skip_sessions: &BTreeSet<u32>) -> Vec<u32> {
    let line = chain(by_pid, peer, stop);
    let sessions: Vec<u32> = line.iter().filter_map(|p| by_pid.get(p)).map(|p| p.sid).filter(|s| *s > 1 && !skip_sessions.contains(s)).collect();
    let mut seen = BTreeSet::new();
    line.into_iter().chain(sessions).filter(|p| seen.insert(*p)).collect()
}

fn tags_of<'a>(by_pid: &BTreeMap<u32, &'a Proc>, pid: u32) -> impl Iterator<Item = (&'a str, Tag)> {
    let owners = by_pid.get(&pid).and_then(|p| p.owners.as_deref()).unwrap_or("");
    owners.split(',').map(str::trim).filter_map(|raw| parse_tag(raw).map(|t| (raw, t)))
}

/// Who `peer` is for `hub` (pure).
pub fn judge(procs: &[Proc], peer: u32, hub: &HubSide) -> Who {
    let by_pid: BTreeMap<u32, &Proc> = procs.iter().map(|p| (p.pid, p)).collect();
    if by_pid.get(&peer).is_none_or(|p| p.zombie) {
        return Who::Gone;
    }
    // the hub and its parents: the hub's own children (its switch, its
    // sb-core) stop there, and its starters are its owners
    let own: BTreeSet<u32> = chain(&by_pid, hub.pid, &BTreeSet::new()).into_iter().chain([hub.pid]).collect();
    for pid in walk(&by_pid, peer, &own, &own) {
        for (raw, t) in tags_of(&by_pid, pid) {
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

/// Who opened a connection to a machine-wide service.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Peer {
    /// Not an agent's process: the user's.
    Outside,
    /// A process of an agent of any hub: its tag.
    Agent(Tag),
    /// The process is gone or could not be read: refused (fail closed).
    Gone,
}

/// Who `peer` is for a machine-wide service `me` (the computer-use
/// broker's pid), pure: any hub's agent is an agent, wherever its hub.
///
/// The key rule: **the last tag of the nearest tagged process**. The walk
/// is [`judge`]'s (the peer, its parents up to pid 1 or `me`, then their
/// sessions but `me`'s); the first process with a tag decides, and the
/// last tag of its list is the agent: a REPL's list is the outer hubs'
/// tags then its own, so an agent of a throwaway hub that another agent
/// started is the inner one. A process with an empty list (opted out)
/// says nothing: its parents decide.
pub fn judge_any(procs: &[Proc], peer: u32, me: u32) -> Peer {
    let by_pid: BTreeMap<u32, &Proc> = procs.iter().map(|p| (p.pid, p)).collect();
    if by_pid.get(&peer).is_none_or(|p| p.zombie) {
        return Peer::Gone;
    }
    let stop: BTreeSet<u32> = [me].into();
    let skip: BTreeSet<u32> = [me].into_iter().chain(by_pid.get(&me).map(|p| p.sid)).collect();
    for pid in walk(&by_pid, peer, &stop, &skip) {
        if let Some((_, t)) = tags_of(&by_pid, pid).last() {
            return Peer::Agent(t);
        }
    }
    Peer::Outside
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

    // ---- judge_any: the computer-use broker (docs/issues/18) ----

    /// The broker: pid 950, detached (under launchd, its own session, no
    /// tags), in [`world`].
    fn broker() -> (Vec<Proc>, u32) {
        let (mut procs, _) = world();
        procs.push(p(950, 1, 950, Some("")));
        (procs, 950)
    }

    fn agent(t: &str) -> Peer {
        Peer::Agent(parse_tag(t).unwrap())
    }

    /// The law table of judge_any: who may run the user's commands
    /// (Outside), and which agent each other peer is (the last tag of the
    /// nearest tagged process).
    #[test]
    fn judge_any_law_table() {
        let (mut procs, me) = broker();
        // a throwaway hub tt started by agent y of aa (BISE_OWNERS of its
        // REPL = [outer, inner]), its REPL 610 and that REPL's nc 611; the
        // test hub's TUI 612, started by y's test 601
        procs.push(p(500, 100, 500, Some("aa.y.1")));
        procs.push(p(601, 500, 500, Some("aa.y.1")));
        procs.push(p(600, 601, 600, Some("aa.y.1")));
        procs.push(p(610, 600, 610, Some("aa.y.1,tt.z.2")));
        procs.push(p(611, 610, 610, None));
        procs.push(p(612, 601, 500, Some("aa.y.1")));
        // a double fork out of t1's REPL session that hides its env
        procs.push(p(203, 1, 200, None));
        // a process that opted out (BISE_OWNERS=) under t1's bash
        procs.push(p(206, 201, 206, Some("")));
        // a process the broker started (the helper's `open -g`)
        procs.push(p(951, 950, 950, None));
        let laws: &[(u32, Peer, &str)] = &[
            (301, Peer::Outside, "the user's TUI"),
            (202, agent("aa.t1.5"), "a hidden-env process below a REPL"),
            (203, agent("aa.t1.5"), "a double fork still in the REPL's session"),
            (402, agent("bb.x.9"), "another project's agent: any hub's agent is an agent"),
            (611, agent("tt.z.2"), "a throwaway hub's agent: the inner (last) tag"),
            (612, agent("aa.y.1"), "a test hub's TUI started by an agent: refused the user's commands"),
            (206, agent("aa.t1.5"), "an opted-out process: its tagged parent decides"),
            (951, Peer::Outside, "the broker's own child: the walk stops at the broker"),
            (999, Peer::Gone, "a gone peer fails closed"),
        ];
        for (pid, want, why) in laws {
            assert_eq!(&judge_any(&procs, *pid, me), want, "{why} (pid {pid})");
        }
        procs.push(Proc { zombie: true, ..p(207, 201, 200, None) });
        assert_eq!(judge_any(&procs, 207, me), Peer::Gone, "a zombie fails closed");
    }

    /// A broker that was not detached (its parent an agent's MCP server,
    /// in that agent's session) still sees that agent's other processes
    /// as the agent's: the walk stops at the broker only.
    #[test]
    fn judge_any_does_not_skip_the_brokers_parents() {
        let (mut procs, _) = world();
        procs.push(p(960, 201, 960, Some("aa.t1.5")));
        assert_eq!(judge_any(&procs, 202, 960), agent("aa.t1.5"));
    }
}
