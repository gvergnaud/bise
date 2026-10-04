# Tech debt

Task `tech-debt`, BISE-292. A ranked list of what to improve in the code,
the tests and the tooling, read at f72cc2b. Each item: what and where, why
it hurts, the fix, cost (S < 1 h, M < 1 day, L more), risk, and its kind:

- **cheap**: obvious and cheap. S, low risk, no behavior or UX change, no
  public interface change. The `tech-debt` task fixes these itself.
- **major**: anything else (architecture, a refactor over ~300 lines, a
  behavior or UX change, a new dependency, deleting a feature, a change of
  the test infrastructure). Asked to the user first, through main.

Ranked by value: risk removed per hour spent.

## What is fine already

- Dependencies: no crate in a `[dependencies]` table is unused (a scan of
  each crate's sources for every dependency name).
- Lints: 3 `#[allow]` in the whole workspace (`core.rs:342` with a reason;
  `chrome.rs:406`, `panel.rs:127` `too_many_arguments`). Clippy runs with
  `-D warnings` in every gate.
- Links: after BISE-287/290 one url scanner is left (`tui/src/links.rs:94`),
  one text layer draws them.
- The repo tracks no build output, `__pycache__` or `.DS_Store`.
- The `-pure.bend` twins of `runtime/*.bend` are not copies (95 of 749
  lines shared for `main`): the PROOF gate needs the pure half apart.

## The list

### 1. The tests' waits: two poll loops, only one knows the load — major (S)

- **where:** `tests/tui_tmux.py:32` `wait_until` scales its timeout by
  `load_factor()` (load average per core, 1 to 4); `tests/e2e.py:164`
  `Env.wait` does not, and `home_migrate.py:67,94` have their own fixed
  loops (15 s, 30 s).
- **why:** the known flaky timeouts (home_migrate, proc_cleanup, and the
  e2e-based ones) fail when 3 to 5 agents build at once: every rerun passes
  alone. Each flake costs a full-gate rerun (~155 s) and an agent's doubt.
- **fix:** `Env.wait` uses the same `timeout * load_factor()` (moved to
  e2e.py, `tui_tmux` imports it); `home_migrate.py` uses `Env.wait`-style
  helpers. A test that passes returns as soon as today.
- **risk:** low. A broken test waits up to 4x longer before failing.
- **kind:** major only because it changes the shared test helper.

### 2. `tui_keys_tmux`: the provider list is walked with a blind 0.15 s — cheap (S)

- **where:** `tests/tui_keys_tmux.py:72-80`: press Down, sleep 0.15 s,
  read the screen, 20 times.
- **why:** under load the screen is read before the TUI drew the last Down;
  the next Down overshoots the row and the loop runs out: `no row for …`.
  One of the known flaky tests.
- **fix:** after each Down, wait until the highlighted row changes (the
  `wait_until` of tui_tmux), then decide.
- **risk:** none outside the test.

### 3. `at_popup_tests`: a 2 s bound on a background walk, shared picks — cheap (S)

- **where:** `tui/src/at_popup_tests.rs:40-50` waits at most 200 × 10 ms
  for the file index walked in the background; `files.rs:544` keeps the
  recent picks per process, shared by the tests running in parallel.
- **why:** under load the walk takes more than 2 s; the test then runs on
  an empty index and fails (`browse_into_folders_then_pick_a_file`,
  `inline_browse_keeps_the_rest_of_the_line`: 1 run in 3 on a loaded
  machine, see the inbox-wrap, license and help-legend threads).
- **fix:** wait up to 30 s (returns at once when ready) and fail with a
  clear message; the assertions that depend on the order already compare
  as a set where a recent pick may come first, check the others do too.
- **risk:** none outside the tests.

### 4. The hub panics when sb-core dies — major (M)

- **where:** `switchboard/src/core.rs:505-513` `CoreLink::call`:
  `expect("sb-core: write")`, `expect("sb-core: read")`, `panic!` on a bad
  answer. Every hub input goes through it (`core.rs:650,679,690,700,1033`).
- **why:** sb-core crashing (an OOM, a Bend runtime error, the EDR killing
  it) takes the whole hub down with every agent's connection, with a Rust
  panic in hub.err as the only trace. The same class as BISE-291: the user
  sees bise stop, not why.
- **fix:** `call` returns a `Result`; on an error the hub logs it, restarts
  sb-core and replays its state (`replay` exists, `core.rs:690`), and says
  so in main's feed.
- **risk:** medium: the replay path must give the same state.
- **kind:** major (behavior: a new recovery path and a feed line).

### 5. The hub writes an agent's files and ignores the errors — cheap (S)

- **where:** `switchboard/src/daemon.rs`, 55 `let _ =`. Most are fine
  (best-effort cleanup). Four are not: `:937` `role.md` (the agent starts
  without its role), `:977-979` the fresh `wire.log`/`wire.offset` (the
  hub then reads an old wire log), `:368-369` the wire offset, `:1524`
  `role.json`.
- **why:** a full disk or a permission problem gives an agent with no role
  or a replayed old turn, and nothing in hub.log: a silent failure like
  BISE-291.
- **fix:** these writes log their error to hub.log (`log_line`), as the
  rest of the daemon does. No behavior change.
- **risk:** none.

### 6. Commits from a private index: nothing checks what they undo — major (S)

- **where:** the agents' commit recipe (a private `GIT_INDEX_FILE`), written
  in each brief, done by hand.
- **why:** b4c2d14 (BISE-287) was built from a private index that held an
  older tree: it undid BISE-285 and BISE-286; f3ff540 undid BISE-197 the
  same way. Each time a second commit put the work back.
- **fix:** `scripts/commit-mine.sh <msg> <paths…>`: builds the index from
  the current HEAD, adds only the paths given, shows
  `git diff --cached --stat HEAD`, refuses when a path outside the list
  changes, commits. The briefs name it.
- **risk:** low. **kind:** major (it changes the agents' workflow).

### 7. `kill` and `ps` by process, in 6 places — cheap (S)

- **where:** `daemon/repl.rs:24-43` (`kill_pid`, `pid_alive`, `is_repl`),
  `switch.rs:104-111,142,217`, `daemon.rs:1539-1549`, `plugins/src/bridge.rs:338`,
  `tui/src/term.rs:374,1038`. `procs.rs` already calls `kill(2)` directly.
- **why:** each check spawns a process from the hub (`kill -0` in a poll
  loop, `switch.rs`). Every spawn from a hub thread is a window for the
  BISE-291 race (a descriptor not yet close-on-exec leaking into a
  concurrent child), and costs ~2-5 ms. Five copies of the same helper.
- **fix:** `procs::{alive, terminate}` over `kill(2)` (the `extern` is
  there), used by the hub's modules. The `ps` calls stay (they read
  command lines) but go through one helper.
- **risk:** low: same signals, same results. `plugins` and `tui` do not
  depend on `switchboard`: they keep theirs.

### 8. The local time comes from `date`, three ways — cheap (S)

- **where:** `tui/src/feed.rs:1358` `local_hhmm` (one `date +%H:%M` per
  pause mark), `tui/src/when.rs:102` `offset_at` (cached per hour),
  `harness/src/update.rs:321` (`date -u`, UTC: no process needed).
- **why:** two ways to get the zone in the same crate; `local_hhmm` spawns
  a process from the UI loop (`run.rs:40`).
- **fix:** `local_hhmm` = UTC now + `when::offset_at(now)`; `update.rs`
  formats UTC itself.
- **risk:** low (the feed tests pin the format).

### 9. Giant functions — major (L each)

- **where:** `harness/src/main.rs:491` `main` (494 lines),
  `switchboard/src/daemon.rs:1580` `run` (411), `tui/src/ui.rs:33`
  `draw_bise` (278), `switchboard/src/cli.rs:279` `build` (227),
  `catalog/src/lib.rs:501` `apply` (196), `tui/src/feed.rs:464`
  `push_event` (196), `tui/src/input.rs:502` `on_key` (189). Files:
  `tui/src/onboarding.rs` 2494 lines, `tui/src/sb/panel.rs` 2304,
  `switchboard/src/daemon.rs` 2020, `bend/hub/core.bend` 2419.
- **why:** every task touches `daemon.rs::run` and `input.rs::on_key`:
  merge conflicts between parallel agents and changes whose reach nobody
  can see at once (BISE-291's watchdog went into `run`).
- **fix:** one at a time, when a task works there anyway: `daemon::run`
  one function per message kind (it is a `match` on `Msg`); `main`'s
  subcommands one function each.
- **risk:** medium (behavior-neutral, but large diffs next to live work).

### 10. `tui_term_tmux` runs alone, cause unknown — major (M)

- **where:** `tests/run_all.sh:43` `ALONE="tui_term_tmux"`: its Ctrl+U
  sometimes leaves the composer text under parallel load (2 of 5).
- **why:** it adds its whole run time to the full gate's tail, and the
  race it hides may be a real one (a key lost between the terminal's pty
  and the composer).
- **fix:** find the cause first (a keystroke before the focus moved back
  to the composer is the likely one), then run it in parallel.
- **risk:** low; the time is the cost.
- **done (docs/issues/10-tests-wait.md):** no race in the TUI. The old
  needle `to the composer` sat in the key bar's tip `ctrl+r speaks into
  the composer`: after Ctrl+U the composer is empty and the bar shows its
  tip, so `wait_gone` failed. The tip is a random one of 10 per session
  (`keybar::current_tip` seeds with the clock), so it failed now and then
  whatever the load (2 of 5 parallel vs 0 of 5 alone was a small sample).
  a361130f changed the needle and left `ALONE=`; `ALONE=` is gone, the
  test runs with the others.

### 11. The workspace id is computed in 4 places — cheap (S)

- **where:** `switchboard/src/paths.rs:30` (the source), and Python copies
  in `tests/gate.sh` (`gate.sh new/done`), `tests/worktree_home.py`,
  `tests/proc_cleanup.py`.
- **why:** a change of the Rust id silently sends `gate.sh new` to another
  folder than the hub's (orphan worktrees nobody removes).
- **fix:** a unit test pins `workspace_id` on 3 paths with the values the
  Python copy gives, and the copies name that test.
- **risk:** none.

### 12. Other notes (no action proposed now)

- `lock().unwrap()`: 29 sites outside tests. A panic under a lock poisons
  it and cascades; acceptable while panics are bugs.
- The release binary is 9 MB; the gate seed ~2 GB of shared blocks. Fine.
- `#[allow(clippy::too_many_arguments)]` at `chrome.rs:406` and
  `panel.rs:127` have no reason; a struct would do, when someone works
  there.

## Progress

Batch 1 (the cheap items), quick gate per commit, one full gate for the
batch:

| item | commit | what |
|---|---|---|
| 2 | ab29f65 | tui_keys_tmux waits for each Down to be drawn |
| 3 | ab29f65 | at_popup_tests waits up to 30 s for the file index |
| 5 | 9471f74 | `write_logged`/`rename_logged`: the hub logs a failed write of role.md, wire.log, wire.offset, role.json |
| 7 | dce3875 | `procs::{alive, terminate, kill_now}` over kill(2) in daemon/repl.rs, switch.rs, `kill_stale_repls` (the `ps` reads stay) |
| 8 | 408902e | `feed::local_hhmm` from `when::offset_at`; `update.rs`'s `date -u` stays (one call per update) |
| 11 | 104eec8 | `paths::tests::ids_match_the_python_copies`; gate.sh and worktree_home.py name it |

The user's decisions on the major items: 1 and 4 go (next); 6 no (not
convinced, portability); 9 and 10 not now (kept here).

Batch 2 (the major items the user said go for):

| item | commit | what |
|---|---|---|
| 1 | a76aa6b | `e2e.load_factor()`: `Env.wait`, `tui_tmux.wait_until` and home_migrate's loops scale their timeout by the load |
| 4 | (this batch) | `Hub::call`: a dead sb-core is restarted on the journal (`Revive`, set by the daemon), the REPL states put back (`force_run`), the input run again once (dropped if it kills sb-core twice), `sb warn` in main's feed, hub.log; more than 3 restarts in 60 s stop the hub as before. Tests: `core_tests::{a_dead_sb_core_is_restarted_on_the_journal, a_crash_loop_of_sb_core_stops_the_hub}`, e2e `core_restart.py` (sb-core killed -9 under a live hub) |

What a restart loses (runtime only, not in the journal): the `sb ask`
waits in flight (their callers time out), the steer counters, a turn's
start time. The durable state and the REPLs are untouched.

A flake the batch's full gate found (2 runs of 2), not in the known list:
`tui_demo_tips_tmux` typed the three spawns while main was still in a
turn on `other`'s ack; the fake only acked them and dev-api never came.
The test now waits for main idle 2 s in a row first (4 of 4 in parallel).

Batch 3 (docs/issues/10-tests-wait.md, agent tests-wait): items 1, 2, 3
and 10 for good.

| item | commit | what |
|---|---|---|
| 1 | 0799998e | `tests/wait.py` is the one way a test waits (`until`, `holds`, `stable`, `load_factor`); `Env.wait`, `wait_until` and the per-file loops call it; the e2e blind waits wait for state |
| 1, 2 | fff86261 | the tmux tests' blind waits: `Tui.sync()` sentinel, `Tui.press_until()` (each key drawn before the next), drawn states |
| 3 | fff86261 | `at_popup_tests::rows_read` waits up to 30 s too |
| 1 | 5490b106 | `tests/sleep_check.py` (run_all.sh, gate.sh quick) refuses a new blind `time.sleep(` outside wait.py and `tests/sleep_exceptions.txt` |
| 10 | (this batch's last) | `ALONE=` gone: the old needle sat in a key-bar tip (item 10 above) |
