# 14 · a TUI test under tmux can run another tree's runtime

Status: open. Found by `interrupt` (issue 13) while two gates ran at once. Label: tests.

## The problem

A `tui_*_tmux` test is supposed to test the tree it runs from. When another gate is running in parallel, it can use that gate's runtime instead.

Repro (interrupt, m_10087):
- gate 1 runs on tree `gwt-g1b`, built from 28263efc (before the interrupt fix); its tmux tests are running;
- at the same time, a one-off `tui_tmux` session runs from tree `gwt-g8`, built from c4768dee (with the fix);
- the one-off's hub writes `hub.root = …/tmp/gwt-g1b` and runs `gwt-g1b/repl-live`, so ctrl+c takes 17-21 s instead of 0.4 s;
- passing `BISE_APP_ROOT=<gwt-g8>` in `tui_session(env=...)` fixes it.

So a test can pass or fail on another tree's code. A test of the wrong tree gave a false red here, but the opposite (a false green on a change under test) is just as possible.

## What is known about the cause (not settled)

- interrupt's reading: the tmux server's global env carries `BISE_APP_ROOT` from whichever gate started the server, and the test's command does not override it.
- Against that: `tests/tui_tmux.py` `start_tui` runs `env -u <every name in bise_env.NOT_INHERITED> … switchboard`, and `BISE_APP_ROOT` is in `INTERNAL`. So a server-wide `BISE_APP_ROOT` should be dropped before the TUI starts.
- Another possible path: the app root comes from the executable when `BISE_APP_ROOT` is unset (`rust/harness/src/approot.rs`: the exe's 3 parents, then, in a debug build, the tree it was built from via `CARGO_MANIFEST_DIR`). `e2e.EXE` is `$CARGO_TARGET_DIR/debug/bise`. A shell that kept gate 1's `export CARGO_TARGET_DIR=<gate 1's target>` (gate.sh prints that line) runs gate 1's `bise`. Its root is then gate 1's tree, with no tmux involved.
- To tell them apart: in the failing setup, print `e2e.EXE`, its `realpath`, and `tmux show-environment -g BISE_APP_ROOT`.

## The result (what is true after)

1. Every TUI test under tmux runs the runtime and the `bise` of the tree it is in, whatever other gates run at the same time.
2. If it can't, it fails at start and names the root it got, instead of testing other code.

## Proposed fix (small, works whichever cause it is)

- `start_tui` passes `BISE_APP_ROOT=e2e.ROOT` explicitly (after the `-u` list), as interrupt's workaround did.
- `e2e.EXE` must be under `e2e.ROOT` or under a `CARGO_TARGET_DIR` that this tree's gate made. If not, it fails with one line naming both paths.
- `tui_session` checks the hub's `hub.root` == `e2e.ROOT` once the hub is up, and fails with both paths if not.
- The tmux tests use their own server (`tmux -L bise-<pid>`, killed at the end), as `proc_cleanup.py` and `agent_tmp_e2e.py` already do. Gates then never share a server or its env.

## No-regression check

- A new check in `tests/tui_tmux.py`'s self-test: with `BISE_APP_ROOT=/elsewhere` set on the default tmux server and `CARGO_TARGET_DIR` pointing at another tree's target, `tui_session` either runs this tree's root or fails at start naming the wrong one. It never runs the other tree.
- Two full gates at once on two trees: green on both (under `/usr/bin/lockf -k ~/.bise/run/heavy.lock nice -n 10` they run one after another, so check this once with the lock off, on a charger).
- `tests/tui_tmux.py` and 5 `tui_*_tmux` tests green alone.
