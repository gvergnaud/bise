# 10 · Tests wait for state, never a fixed time: one wait helper, no blind sleeps, no test run alone

Status: in progress (agent tests-wait). Root node 2 of the architecture page's tech debt list; label: tech-debt.

## The problem

Most of our flaky tests have one cause: a test waits a fixed time and then checks, instead of waiting for the state it needs. On a busy machine (several agents building at once) the state comes later than the wait, and the test fails. A rerun alone passes.

Evidence:
- `tests/` has 112 `time.sleep(` calls in 60 files. Some are poll intervals inside a wait loop (fine). Others are blind waits before a check: `tests/every_e2e.py` sleeps 12 s then 6 s, `tests/plugins_reload_e2e.py` 5 s, `tests/subscriptions_tui_tmux.py` 4 s, `tests/tui_computer_use_tmux.py` 3 s, and many 0.3 to 1.5 s waits after a key in the tmux tests.
- There are three ways to wait: `e2e.Env.wait`, `tui_tmux.wait_until` and per-file helpers (`wait_composer`, `wait_clip`, `idle_exit_e2e.wait`...). Only the first two scale with `load_factor()`.
- `tests/run_all.sh` runs `tui_term_tmux` alone (`ALONE=`) because it fails in parallel for a cause nobody found.
- The same fix was made test by test, again and again: `docs/tech-debt.md` items 1, 2, 3 and 10; commits ab29f655, dccd7b2c, 53a8e4e2, e0acca79, 42a9ae09. `t_escalation_card` in `tests/e2e.py` failed once in a full gate and passed alone twice.
- Each flake costs a full gate rerun (about 155 s) and makes an agent doubt a change that was fine.

## The result (what is true after)

1. One wait module (`tests/wait.py`, or in `e2e.py`) is the only way a test waits: it polls a condition until a deadline scaled by `load_factor()`, and on timeout it fails with what it waited for and what it saw last.
2. No blind wait: every `time.sleep(` in `tests/` is inside that module or in a list of reviewed exceptions (a sleep that is the thing under test, such as a fake process that must stay alive). A check in `tests/run_all.sh` and `tests/gate.sh quick` fails on a new one, and its error names the helper to use.
3. The tests that waited for time wait for state: a line in a feed, an agent's status, a journal line, a file, a drawn screen.
4. `tui_term_tmux` runs with the others: its race is found and fixed (in the test or in the TUI), and `ALONE=` is empty or gone.

## No-regression check (anyone can run it)

- `tests/run_all.sh` green, with the same test list as before (no test removed or weakened: each changed test keeps its assertions).
- New: the sleep check itself, proven by a test that it fails on a file with a blind `time.sleep(2)` and passes on the wait module.
- Stability: `tests/run_all.sh` passes 5 times in a row with `SB_TEST_JOBS=4` while a `cargo build` runs beside it (the load that made the flakes), and `tui_term_tmux` passes 10 times in a row in parallel.
- Speed: the full gate is not slower than before (a wait returns as soon as its state is there): the run time is in the report, before and after.

## Scope

- `tests/*.py`, `tests/run_all.sh`, `tests/gate.sh`, the Rust test files with a wait loop (`rust/tui/src/at_popup_tests.rs`).
- A fix in product code only if the race is in the product (`tui_term_tmux`'s lost key, if it is one), in its own commit.

## Out of scope

- New tests, other refactors of the tests.
- Changing the hub's protocol to help tests. If a test cannot see the state it needs, list it in the report instead.
