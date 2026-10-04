# 10 · Tests wait for state, never a fixed time: one wait helper, no blind sleeps, no test run alone

Status: done (agent tests-wait), with one check not run (below). Root node 2 of the architecture page's tech debt list; label: tech-debt.
Commits: 0799998e (tests/wait.py, the e2e waits), fff86261 (the tmux waits, at_popup_tests), 5490b106 (tests/sleep_check.py + sleep_exceptions.txt in run_all.sh and gate.sh quick; the duplicate tui_checker_tmux out of TESTS), and the last one (ALONE= gone, tui_onboarding_tmux's welcome clock, docs/tech-debt.md items 1, 2, 3, 10).
Numbers: `rg -n 'time\.sleep\(' tests/*.py` went from 112 lines to 8: tests/sleep_exceptions.txt's 7 reviewed entries (the fakes' slow replies, the memory hog, the fake browser's 4 s, the fake helper's 3 s, the double-click gap). The check refuses a planted `time.sleep(2)` and passes the tree. tui_term_tmux: cause found (the old needle 'to the composer' sat in a random key-bar tip, fixed by a361130f, ALONE was left), 10 of 10 at SB_TEST_JOBS=4 and 20 of 20 at 4-6 at once. 'Nothing happens' windows on wait.holds (each times load_factor()): about 25 s in the e2e tests (every 6, idle_exit 6, plugins_reload 5, skills_reload 3+3, subscriptions 2) and about 8 s in the tmux tests; the others use a sentinel (Tui.sync()). Full gate: before 680 s (8a0da264, load ~30, red: agent_tmp_e2e, idle_exit_e2e, repl_bash_env, tui_checker_tmux, tui_onboarding_tmux); after 453 s (load 7-20, red: agent_tmp_e2e and repl_bash_env, the bash tool's background handoff, sent to the bg-handoff agent; tui_version_tmux, which wants the tree's HEAD in the version list, so it fails on any unlanded branch commit; tui_onboarding_tmux, fixed in the last commit, then 2 of 2; subscriptions_tui_tmux, 2 of 2 alone after, cause not found). The loads differ, so the two times don't compare well.
Not run: the 5 x run_all.sh with SB_TEST_JOBS=4 beside a cargo build (battery: one heavy job at a time, main m_7694); main asks for it when the machine is on power.

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
