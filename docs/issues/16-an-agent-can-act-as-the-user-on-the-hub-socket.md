# 16 · an agent can connect to the hub's client socket and act as the user

Status: done (socket-auth, architect's plan m_10984; see "What was done" at the end). Found by architect while reviewing amb-feed's typed approvals plan (m_10946). Read from the code, not exploited. Label: security.

## The problem

The hub has one socket, `hub.sock`. `daemon.rs` `accept_loop` reads the first line of each connection:
- `{"op":"hello"}` becomes a **client** (`Msg::ClientNew`): the user's TUI, with the user's authority;
- `{"op":"agent"}` becomes an agent request.

Nothing checks who sent `hello`. Every agent's REPL has `SB_SOCKET` pointing at that same socket (that is how `sb` reaches the hub), and every agent has a shell. So a command an agent runs can do:

```
printf '{"op":"hello"}\n{"op":"approvals","mode":"yolo"}\n' | nc -U "$SB_SOCKET"
```

and the hub takes it as the user. With a client connection, an agent can:
- turn approvals off (`approvals` op, `set_mode`), or remove rules;
- answer its own approval card (`/answer N 1`), or any card;
- send input as the user to main or to any agent;
- archive or stop other agents.

In auto mode the checker sees the bash command, but the escalation can hide in a script file the agent wrote first. In yolo mode nothing stands in the way, but there the user has already given that power.

The typed protocol (bise desktop: `HubCmd::Approvals`, `RemoveRule`, `Answer`, `Confirm`) adds more such commands on the same client path. It doesn't create the gap, but it widens what one connection can do.

## The result (what is true after)

1. A process started by an agent (its bash, its scripts, their children) cannot open a client connection to its hub. Its `hello` is refused with one hub.log line naming the agent.
2. The user's TUI, the desktop app's core, `bise` commands he runs himself and the tests still connect as today.
3. Agents keep everything `sb` gives them (their own requests with their token).

## Ways to get there (to decide in a plan)

- **Two sockets.** `agent.sock` accepts only `{"op":"agent"}` and is what `SB_SOCKET` names. `hub.sock` accepts only clients and is never in an agent's environment. Alone, this only hides the path: an agent can still find `hub.sock` under the hub's state dir.
- **Peer check.** On `hello`, read the peer's pid (`getsockopt(SOL_LOCAL, LOCAL_PEERPID)` on macOS, `SO_PEERCRED` on Linux) and refuse it when it descends from one of the hub's REPLs (the hub knows their pids; walk the parents). A double fork to launchd escapes it, so this is defense in depth, not a wall.
- **Sandbox.** With the approvals sandbox on (`BISE_SANDBOX`), deny agent processes access to `hub.sock` by path. That is the real wall, but only where the sandbox runs.

Likely answer: two sockets + the peer check now, and the sandbox deny wherever the sandbox is on.

## No-regression check

- A new e2e: an agent runs the `printf … | nc -U` command above (through a script file too). Its `hello` is refused, approvals stay as they were, its card stays open, and hub.log names it.
- `tests/e2e.py`, `tests/approvals_e2e.py`, `tests/proto_e2e.py`, the TUI tmux tests and the desktop's real-core checks stay green: every real client still connects.
- One full gate.

## What was done

- **Two sockets.** `agent.sock` (`Paths::agent_socket`, same short-link folder as hub.sock) is every new REPL's `SB_SOCKET`; it serves `agent`, `version` and `ping`, and answers anything else with `agent.sock does not serve the op …`. `hub.sock` serves `hello`, `notice` and `ping` (`crate::peer::access`).
- **Peer check** (`crate::peer::judge`, pure, with a law table; `peer_os.rs` reads the peer pid: macOS `LOCAL_PEERPID`, Linux `SO_PEERCRED`; `daemon/accept.rs` is the shell). On `hello` and `notice`, the hub reads the process table and refuses a peer whose parent chain, or the session of one of them, holds a process tagged (`BISE_OWNERS`) by this hub or by a sibling hub (another project in the same `hubs/` folder). A hub's own owners (the tags it inherited, the processes above it) are not refused, so a throwaway hub an agent starts in a test serves that agent's clients. A peer that is gone or unreadable is refused (fail closed). A refused connection gets `{"ev":"refused","ok":false,"error":"this connection comes from agent <name>'s process: clients must be started by the user"}`, the TUI prints it and exits 1, and hub.log gets `client refused on hub.sock (<op>): pid N (<program>): …`. A double fork that leaves the agent's session and drops `BISE_OWNERS` escapes this check; the sandbox below is the wall.
- **Sandbox.** With the sandbox on, both profiles end with `(deny network-outbound (remote unix-socket (path-literal <hub.sock>)))`, for the natural path and the short one, for every agent whose REPL got agent.sock (`repl.json` `"sock": "agent"`).
- **Transition shim (to remove).** REPLs adopted from an older hub still have `SB_SOCKET=hub.sock`, so hub.sock still serves `agent` and `version` and writes one hub.log line per request (`hub.sock shim: …`). **Remove it in the release after v2026.10.2-25**: `Access::Shim` in `crate::peer` becomes `Access::No`. Check hub.log for `hub.sock shim` lines first: when there are none, it is safe.
- Tests: `peer::tests` (the law table), `peer_os::tests` (the real OS), `sandbox_tests` (the profile, and the real `sandbox-exec` closing hub.sock by path and through a link while agent.sock stays open), `tests/socket_auth_e2e.py` (the agent's `printf | nc`, its `SB_SOCKET`, a script it leaves behind trying to toggle approvals and `/answer` its card: all refused; `sb` and the test's client still work). That e2e was red before the change and is green after.
- Not covered: `from` on agent.sock is still self-declared (issue 17).
- Left (architect, m_11227): a `notice` whose writer exits before the hub's process snapshot reads `Gone` and passes (a notice is refused only on proof; `/restart`'s own notice writes and closes at once), so an unsandboxed (yolo) agent can race an info line into main's thread. The sandbox's deny of hub.sock covers sandboxed agents, and a notice carries no user authority (an info line, never an approval or input). Fix idea if the race ever matters: read the peer's whole chain at accept, not only its pid.
