# 16 · an agent can connect to the hub's client socket and act as the user

Status: open. Found by architect while reviewing amb-feed's typed approvals plan (m_10946). Read from the code, not exploited. Label: security.

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
