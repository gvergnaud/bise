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
//!   `xin`/`xreply`/`xfollow` (hub to hub, judged like `hello`);
//!   `ping`; and, until the release after v2026.10.2-25, agent requests
//!   from REPLs adopted from an older hub, whose `SB_SOCKET` is still
//!   hub.sock (a shim, logged per request).
//!
//! Who is an agent's process is `bise_peer::judge` (shared with the
//! computer-use broker, docs/issues/18): this hub's agent or a sibling
//! hub's (another project's, same `hubs/` folder), unless that tag or
//! process is the hub's own owner (a throwaway hub an agent started lets
//! that agent's processes be its clients). It is defense in depth: the
//! sandbox's deny of hub.sock is the wall.

pub use bise_peer::judge::{judge, HubSide, Who};

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
        // hub to hub (desktop S2, `daemon/xhub.rs`): bise's hub and a
        // project's hub talk on each other's hub.sock, as the user's
        (Sock::Client, "xin" | "xreply" | "xfollow") => Access::User,
        _ => Access::No,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_sockets_serve_their_ops() {
        assert_eq!(access(Sock::Agent, "agent"), Access::Agent);
        assert_eq!(access(Sock::Agent, "version"), Access::Agent);
        assert_eq!(access(Sock::Agent, "ping"), Access::Open);
        for op in ["hello", "notice", "stop_hub", "xin", "xreply", "xfollow", ""] {
            assert_eq!(access(Sock::Agent, op), Access::No, "{op}");
        }
        assert_eq!(access(Sock::Client, "hello"), Access::User);
        assert_eq!(access(Sock::Client, "notice"), Access::User);
        for op in ["xin", "xreply", "xfollow"] {
            assert_eq!(access(Sock::Client, op), Access::User, "{op}");
        }
        assert_eq!(access(Sock::Client, "ping"), Access::Open);
        assert_eq!(access(Sock::Client, "agent"), Access::Shim);
        assert_eq!(access(Sock::Client, "version"), Access::Shim);
        assert_eq!(access(Sock::Client, "whatever"), Access::No);
    }
}
