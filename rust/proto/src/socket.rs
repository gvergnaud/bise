//! What a hub's two sockets serve (docs/issues/16; issue 37 #2, architect
//! m_35407): a static table of (socket, first op), read on each accept
//! thread before any loop message, so a flood of connections never
//! reaches the hub's loop. It is a protocol table, not a decision on
//! facts: who may speak as whom is sb-core's (bend/hub/peer.bend), on the
//! judge's fact (rust/peer judge.rs).
//!
//! - `agent.sock` (the agents' `SB_SOCKET`): agent requests (`agent`,
//!   `version`) and `ping`, nothing else;
//! - `hub.sock` (clients: the TUI, the desktop's core, tests): `hello` and
//!   `notice`, the hub-to-hub `xin`/`xreply`/`xfollow`, `ping`. Never an
//!   agent request: those are agent.sock's.

/// Which socket a connection came in on.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Sock {
    /// `agent.sock`: the agents' `sb`.
    Agent,
    /// `hub.sock`: the user's clients.
    Client,
}

impl Sock {
    /// Its word in sb-core's inputs (`sock`): `agent` or `client`.
    pub fn word(self) -> &'static str {
        match self {
            Sock::Agent => "agent",
            Sock::Client => "client",
        }
    }
}

/// What a socket does with a connection's first op.
#[derive(Debug, PartialEq, Eq)]
pub enum Access {
    /// Served to anyone (`ping`).
    Open,
    /// An agent request (`agent`, `version`): served when sb-core says its
    /// sender may speak as its `from`.
    Agent,
    /// The user's: served only when the judge says the peer is not an
    /// agent's process (the hello gate, rust/peer `Who::refusal_of`).
    User,
    /// Not served on this socket.
    No,
}

/// What `sock` does with a first line whose op is `op`.
pub fn access(sock: Sock, op: &str) -> Access {
    match (sock, op) {
        (_, "ping") => Access::Open,
        (Sock::Agent, "agent" | "version") => Access::Agent,
        (Sock::Client, "hello" | "notice") => Access::User,
        // hub to hub (desktop S2): bise's hub and a project's hub talk on
        // each other's hub.sock, as the user's
        (Sock::Client, "xin" | "xreply" | "xfollow") => Access::User,
        _ => Access::No,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The law of the table, over every (socket, op) pair of the ops a
    /// hub knows and a few it doesn't: an agent request is agent.sock's
    /// only, the user's ops hub.sock's only, ping both, the rest none.
    #[test]
    fn every_socket_and_op_pair() {
        let ops = ["agent", "version", "hello", "notice", "xin", "xreply", "xfollow", "ping", "stop_hub", "", "whatever", "view/call", "view_call", "view/message"];
        for op in ops {
            let (a, c) = (access(Sock::Agent, op), access(Sock::Client, op));
            let want = match op {
                "ping" => (Access::Open, Access::Open),
                "agent" | "version" => (Access::Agent, Access::No),
                "hello" | "notice" | "xin" | "xreply" | "xfollow" => (Access::No, Access::User),
                _ => (Access::No, Access::No),
            };
            assert_eq!((a, c), want, "{op:?}");
        }
        assert_eq!((Sock::Agent.word(), Sock::Client.word()), ("agent", "client"));
    }
}
