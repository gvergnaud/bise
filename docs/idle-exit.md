# idle-exit: a hub nobody looks at stops by itself

Before: a hub ran until killed. An unused workspace kept its sbd, sb-core
and every agent REPL, each with its plugins bridge and MCP servers
(one audit: 5 unused hubs with 28 REPLs, 175 orphan processes).

## What stops when

The hub counts its UIs: the clients that said `hello` on `hub.sock` (the
TUI, the ambient app's core) plus the holds others take on it
(`idle::Holds`: a page's open event stream). When the count is 0:

1. a grace period starts (`idle_exit` in `~/.bise/config.toml`, or
   `$BISE_IDLE_EXIT`: `90`, `90s`, `5m`, `1h`, `off`; default 2 min). A UI
   that comes back (a quick relaunch, a reconnect) cancels it. A hub that
   nobody opens starts alone: the grace runs from its boot.
2. after the grace, the hub waits while something runs: an agent
   mid-turn (also inside `sb wait`), a background job of an agent's bash
   tool (`<agent>/tmp/bg/<n>.slot` with a live `<n>.pid`, at most 6 h
   old: a forgotten dev server does not hold it forever), a REPL starting
   or switching, a version switch on probation (its switcher read the
   idle exit as a crash and rolled back), a `/version` build, `/update`,
   `/release-bise`. hub.log
   says what it waits for, once per change.
3. then it stops for good, the stop of `bise --stop`: the socket goes
   first (a `bise` launched meanwhile starts the next hub, which waits for
   this one's process to be gone), each REPL gets `reload` (it
   checkpoints its session and exits 0, like a version switch), the ones
   still there after 5 s get SIGTERM, then everything the agents started
   (procs.rs, docs/proc-cleanup.md): plugins bridges, MCP servers, tmux
   servers, test hubs. sb-core dies with the hub.

hub.log, in order: `idle exit: after 120 s without a UI, once nothing
runs` (boot), `idle exit: no UI left, ...`, `idle exit: no UI, the hub
waits for: t1 mid-turn`, `idle exit: no UI for 131 s and nothing runs:
the hub stops`, `hub stop`, `REPLs saved and gone: 2 of 2`.

The next `bise` in the workspace starts a hub as before: the journal
brings back the agents, cards and messages, each REPL restarts on its
saved session (`BEND_CONTINUE`), the transcripts the feeds.

## Children die with their parent

- **REPL -> hub.** A hub killed or crashed leaves its REPLs for the next
  hub to adopt. If none comes: repl-live looks at its hub every
  `BEND_HUB_GONE_MS / 10` (default every minute; `hub.pid` next to
  `$SB_SOCKET`, `kill -0`) and exits after ten looks in a row without it
  (`hub-gone-exit` in repl.log). A switch's hub is back in seconds. Its
  session is saved at each turn boundary and before each provider call.
  No `SB_SOCKET` (a bare repl-live, tests): no watch.
- **plugins bridge -> REPL.** `bise plugins serve --parent <pid>` checked
  its parent with a `kill` process every 500 ms and took a reused pid for
  its parent: bridges outlived their REPL. Now kill(2), and every 10 s the
  parent's start time must still be the one it had (`bridge::Parent`).
- **MCP servers -> bridge.** The bridge stops them when it goes; a bridge
  killed hard leaves them to the hub's reap (they carry the agent's tag).

## Standing orders (`sb every`, ambient-app)

Not on main yet. The rule chosen for the follow-up: a hub with a live
standing order does not stop by itself (one more reason in `idle_busy`).
Simpler and safe: a timer always fires on time, nothing to install or
clean up in launchd, no headless relaunch that boots every REPL to send
one wake. The cost: a hub with an endless daily order stays up, which is
what that order asks for; most orders have `--until` or `--times`.

## Tests

- `idle::tests`: the grace (env, config, `off`), the wait for what runs,
  a UI back cancels, a hub nobody opens stops, holds, background jobs.
- `bridge::parent_tests`: a reused pid is not the parent.
- `tests/idle_exit_e2e.py` (run_all): a real hub on the fake provider,
  grace 4 s; a UI back within the grace keeps the hub; the last UI
  leaves mid-turn, the hub waits for the turn, then stops with both REPLs
  saved; the next start has main and t1 back with their feeds, and t1
  answers; a background job holds the hub until it ends on its own; a hub
  killed -9 leaves REPLs that exit by themselves, and the next hub brings
  them back.
