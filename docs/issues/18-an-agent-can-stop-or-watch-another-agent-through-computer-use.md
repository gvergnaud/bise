# 18 · an agent can stop or watch another agent through computer use's control socket

Status: fixed by step 2 (points 1 and 2 below); step 3 (names) and step 4 (`pause`) done; `peek` follows. Found by architect while reviewing amb-core's plan for computer use live in the desktop window (m_12177, answer m_12179). Read from the code, not exploited. Label: security. The plan, signed by architect (m_12219), in five steps:

1. `rust/peer` (bise-peer): the process table, the peer's pid, `judge` and the new `judge_any`, moved out of switchboard with no copy (9534a310). Done.
2. The broker judges every connection (`computer-use/src/who.rs`): commands move to their own socket, `computer-use-ctl.sock` (its name in `bise_home::socket`), served to the user's processes only and denied in the agents' sandbox; an agent is keyed `<hub id>.<dir>` by its process's tag, never by its hello; a browser link must come from outside; `state.json` v2 and `events.jsonl` carry the key, the name and the hub id, and the TUI and the hub's feed show their own hub's agents only; the broker starts with no `BISE_OWNERS`; the extension maps its groups to keys in session storage. Done (before it, 40a8de09 split broker.rs).
3. The names at the edges: a group title that two live agents share names its project; the helper's cursor pill shows the name (9e05ca4d). Done.
4. `pause` (take over from the window): ctl `{"op":"pause","args":{"agent":<key>}}` on an agent that drives something marks all its targets paused, sends `{"pause":<key>}` to the browsers and the helper (the debugger detaches, the tabs count as the user's, the cursor hides), fails its waiting calls with `paused` and writes the event `paused` by `you`; `resume` hands it back. An agent that drives nothing: `not_found`. Done.
5. `peek` (a still on demand, inline, never stored, only while the agent drives).

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

Two more, found while planning the fix (computer-use, m_12216):

- **The agent role is not checked either.** Any process can say `{"op":"hello","agent":"perf"}` and drive perf's tabs, or read them (snapshot, screenshot): the broker takes the name from the hello.
- **The broker inherits the first agent's tags.** `client::spawn_broker` starts it from that agent's MCP server, so it carries the agent's `BISE_OWNERS`: the hub's process cleanup kills it when that agent is archived, and a judge that skips its own owners' tags would let that agent through.

This is the same hole as issue 16, one socket further. It gets worse with what the desktop app wants next: a take-over op (`pause`) and a live picture of what an agent drives (`peek`), both on `ctl`.

## The result (what is true after)

1. A `ctl` connection from a process of any hub's agent is refused, with one line the caller prints. The user's own commands (his shell, the TUI, the desktop core) keep working. An agent keeps its `agent` role for its own driving.
2. The broker keys each agent by the owner tag its REPL already carries (`BISE_OWNERS`: the hub id plus the agent's folder), with its name as a field. `ctl` ops name the tag, never a bare name. The TUI and the desktop core show only their own hub's agents, or name the project.
3. No `pause` or `peek` op ships before 1 and 2.

## Ways to get there (to decide in a plan)

- **The same judge as issue 16.** `crate::peer::judge` with `peer_os.rs` (switchboard) already tells whether a peer process is an agent's, from its pid and the process table. Move the pure judge and its OS part to a home both crates use (`bise_home`, or a small `bise-peer` crate) with no copy, and call it at the broker's `ctl` hello.
- **The tag at hello.** The MCP server reads `BISE_OWNERS` from its environment and sends it in its hello. The broker keys `state.json` by it. The readers (`parse_state`, the desktop core) map hub id to project through `bise_home`'s `hub_id` and `projects`.

Owner: computer-use (`broker.rs`, the MCP server, the extension), reviewed by architect. On main: the broker ships with the terminal bise. The desktop part lands on ambient-app after.
