# 18 · an agent can stop or watch another agent through computer use's control socket

Status: open. Found by architect while reviewing amb-core's plan for computer use live in the desktop window (m_12177, answer m_12179). Read from the code, not exploited. Label: security.

## The problem

The computer-use broker is one process per machine, on `~/.bise/run/computer-use.sock`. A connection says its role in its first line (`rust/computer-use/src/broker.rs`):

- `{"op":"hello","role":"agent",...}`: an agent's MCP server, which drives its own tabs and apps;
- `{"op":"hello","role":"ctl"}`: a command, `bise computer-use stop|resume|drop|release|status|show`. It is meant for the user.

The broker never checks who opened a `ctl` connection. Any process on the machine can get it, an agent's bash included:

```
bise computer-use stop perf      # another agent's driving stops
bise computer-use resume perf    # or resumes after he took it over
bise computer-use status         # what every agent drives, and where
```

There's a second problem. The broker keys its agents by the name their MCP hello gives. Two projects can each have a `main` or a `perf`: they share one key in `state.json`, one overwrites the other, and a `stop perf` can reach the wrong project's agent. The TUI's driving line (`tui/src/computer_use.rs` `parse_state`) reads the same key and can show another project's agent.

This is the same hole as issue 16, one socket further. It gets worse with what the desktop app wants next: a take-over op (`pause`) and a live picture of what an agent drives (`peek`), both on `ctl`.

## The result (what is true after)

1. A `ctl` connection from a process of any hub's agent is refused, with one line the caller prints. The user's own commands (his shell, the TUI, the desktop core) keep working. An agent keeps its `agent` role for its own driving.
2. The broker keys each agent by the owner tag its REPL already carries (`BISE_OWNERS`: the hub id plus the agent's folder), with its name as a field. `ctl` ops name the tag, never a bare name. The TUI and the desktop core show only their own hub's agents, or name the project.
3. No `pause` or `peek` op ships before 1 and 2.

## Ways to get there (to decide in a plan)

- **The same judge as issue 16.** `crate::peer::judge` with `peer_os.rs` (switchboard) already tells whether a peer process is an agent's, from its pid and the process table. Move the pure judge and its OS part to a home both crates use (`bise_home`, or a small `bise-peer` crate) with no copy, and call it at the broker's `ctl` hello.
- **The tag at hello.** The MCP server reads `BISE_OWNERS` from its environment and sends it in its hello. The broker keys `state.json` by it. The readers (`parse_state`, the desktop core) map hub id to project through `bise_home`'s `hub_id` and `projects`.

Owner: computer-use (`broker.rs`, the MCP server, the extension), reviewed by architect. On main: the broker ships with the terminal bise. The desktop part lands on ambient-app after.
