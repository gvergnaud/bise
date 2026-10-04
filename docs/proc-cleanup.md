# proc-cleanup: an agent's processes die with it (BISE-243)

An agent starts processes through its bash tool: commands, background
jobs (`/tmp/bend-bg-*`), dev servers, test hubs (`bise sbd` + `sb-core` +
`repl-live`), tmux servers, cargo. Before BISE-243 they outlived the agent:
test hubs of finished tasks ran for 11 h, ~20 `repl-live` were reparented
to pid 1, `sbtui*` tmux sessions stayed. Now the hub kills them.

## When

| event | what is killed |
|---|---|
| a task is stopped (`sb stop`, `/stop`) or archived (`/drop`, `sb drop`) | every process that agent started until then (a REPL restored after that is not hit) |
| the hub quits for good (`stop_hub`, not a reload that keeps the REPLs; also its idle exit, docs/idle-exit.md) | every process of every agent |
| the hub starts | the processes of the agents that are not live: stopped, archived, or unknown to this hub (left by an earlier hub) |

A turn interrupt (esc, `sb interrupt`) kills nothing: the agent goes on
and may still use its dev server. A REPL that restarts (crash, reload,
`/version`) keeps its agent's processes.

How: SIGTERM to each, up to 3 s for them to go, then SIGKILL to the ones
still there (same pid and same start time). Off the hub's loop; one line
in `hub.log`: `processes of t1 killed: 9 (47979 bise, 47990 tmux, ...)`.

## How they are tracked (`rust/switchboard/src/procs.rs`)

1. **A tag in the environment.** Each REPL gets `BISE_OWNERS=<hub>.<dir>.<ms>`
   (`<hub>`: FNV-1a of the hub's socket path; `<dir>`: the agent's folder
   name; `<ms>`: the REPL's spawn time). Every process it starts inherits
   it, also one that leaves the process group or session (`setsid`,
   `nohup`, a tmux server, a hub that daemonizes) or is reparented to pid 1.
   A hub an agent starts gives its own REPLs the list it inherited plus its
   own tag (comma-separated), so what those start is the outer agent's too.
   The list is read at kill time from the process table (macOS
   `ps -axww -E`, Linux `/proc/<pid>/environ`): a reused pid has no tag, so
   it is never hit.
2. **The REPL's session.** macOS hides the environment of its own binaries
   (`/bin/sleep`, `/bin/sh`, `/bin/bash`, `/usr/bin/*`: `ps -E` and
   `sysctl(KERN_PROCARGS2)` give only the path). So each REPL runs in a
   session of its own (`setsid`), its pid is appended to its agent's
   `repl.sids`, and a process with no visible list whose session
   (`getsid`) is one of these is the agent's. A session whose leader is
   alive and is not that agent's tagged REPL (the pid was reused) is
   ignored. The file is emptied once its agent is reaped.
3. **The tree.** A child of a hit process is hit too when it has no list
   at all (`env -i`, or hidden).

Never killed: the hub itself and its ancestors (a hub relaunched by one of
its agents carries that agent's tag in its original environment; at its
start the hub drops its own tags from `BISE_OWNERS`, so its sb-core,
builds and REPLs do not carry them), a process whose list names other
agents only, a process with an empty list, and anything untagged outside
the agents' sessions: the user's shells, a bise of another project.

**Opting out.** A process meant to outlive its agent sets `BISE_OWNERS=`
(empty). `scripts/relaunch-live.sh` and `scripts/move-live.sh` do it for
the live hub they start.

## Limits

- On macOS, a system binary (environment hidden) that leaves the REPL's
  session (`setsid`, `start_new_session=True`) and whose parent dies at
  once is not found: `perl -e 'POSIX::setsid(); exec "sleep"...'`. The
  same thing with a Homebrew, cargo or harness binary (tmux, bise, node,
  rg) is found by its tag. macOS's "responsible process" would close this
  (`responsibility_spawnattrs_setdisclaim`), but it moves privacy (TCC)
  prompts to the REPL; not done.
- A tmux session an agent adds to the user's default tmux server (no
  `-L`/`-S`) runs under the user's server, which is not the agent's: it is
  not killed. A server the agent starts (`tmux -L x new -d`, what the tests
  do) is.
- REPLs a hub older than BISE-243 started are not in their own session
  and have no tag: their processes are found only while they are in the
  tree.

Test: `tests/proc_cleanup.py` (in run_all): t1 starts
an orphan `sleep`, a hub of its own (with its main REPL) and a tmux
server; /drop t1 kills them all, t2's and t3's sleeps and the user's stay;
`sb stop t3` kills t3's; the hub quitting kills t2's, never the user's; a
tmux server tagged for an agent the hub does not know dies at its start.
Unit tests: `procs::tests`.
