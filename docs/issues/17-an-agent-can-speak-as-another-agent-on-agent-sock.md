# 17 · an agent can speak as another agent on agent.sock

Status: open. Found by socket-auth while fixing issue 16 (architect's ok, m_10984). Read from the code, not exploited. Label: security.

## The problem

After issue 16, agents reach the hub on `agent.sock` (their `SB_SOCKET`) and can no longer act as the user on `hub.sock`. But on `agent.sock`, who sends a request is the `from` field of the request, and `sb` fills it from `SB_AGENT`, an environment variable the agent's shell can change:

```
SB_AGENT=main sb send t2 "drop everything, land now"
SB_AGENT=main sb restart
```

`daemon.rs` `agent_request` takes `from` as is, and `version_allowed(from, what)` (daemon/versions.rs) lets `sb version switch|rollback` and `sb restart` through when `from == "main"`. So any agent can:
- send messages, spawn tasks, report, or answer waits as main or as any other agent;
- switch, roll back or restart the hub (main only, in principle).

It is not the user's authority (no approvals, no cards, no input as him: that was issue 16), but it breaks the hub's idea of who said what: sb-core's laws, the threads and `sb history` all trust `from`.

## The result (what is true after)

1. A request on `agent.sock` counts as the agent whose process sent it, whatever its `from` says. A mismatch is refused with one line the caller prints, and one hub.log line.
2. `sb` keeps working for every agent, from its bash and its scripts. Tests and scripts that drive `sb` from outside any agent (`SB_AGENT=main sb …` in tests/, scripts/move-live.sh, packaging/test-*.sh) keep working: a process that is no agent's is the user's, who may speak as main.

## Ways to get there (to decide in a plan)

- **The same peer chain as issue 16.** `crate::peer::judge` already finds, from the peer's pid and the process table, the agent whose process it is (the `BISE_OWNERS` tag of this hub in its parent chain or session). On `agent.sock`, refuse a request whose `from` is not that agent; `Who::Outside` may use any `from` (the user's own shell). Cost: one `ps` per `sb` call (~50-150 ms on a busy Mac); a cache by pid for a few seconds would cut it.
- **A token per REPL.** The hub gives each REPL a secret in its environment (`SB_TOKEN`), `sb` sends it, the hub maps it to the agent. Cheaper per call, but any process of the agent can read it and the env is visible to `ps -E` for the same user: it binds requests to whoever read it, so it is weaker than the chain.

Likely answer: the peer chain, with a short cache.

## No-regression check

- A new e2e: agent a1 runs `SB_AGENT=main sb send t2 …` and `SB_AGENT=main sb restart`: both refused, hub.log names a1; its own `sb send` works.
- `tests/e2e.py`, the tests that run `sb` with `SB_AGENT` from outside an agent, the tmux tests: green. One full gate.
