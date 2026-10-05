# 11 · A child process never inherits bise's internal environment variables: one registry, one builder

Status: in progress (the perf fixes shipped in v2026.10.2-19; picked by the user's criterion: the debt that spreads the most, see the evidence below). Root node 3 of the architecture page's tech debt list; label: tech-debt.

## The problem

bise starts many processes: the TUI starts the hub (`sbd`), the hub starts sb-core and one agent REPL per agent, an agent's bash runs `sb` and sometimes another bise (a test hub, a gate). Each child inherits its parent's whole environment, and bise uses about 100 environment variables of its own (`SB_*`, `BEND_*`, `BISE_*`). The hub removes a few by name (`env_remove` in `rust/switchboard/src/daemon.rs`: `SB_CORE_BIN`, `BISE_APP_ROOT`, `BEND_CONTINUE`...). Every variable not on that list goes down to every child. So a value meant for one process silently changes another, and the bug appears far from its cause.

Evidence:
- 5d136677: each hub passed its `SB_CORE_BIN` to the next hub and to the agents, and `sbd` kept the inherited value. A stale sb-core survived version switches, and the journal replay took 10.5 s instead of 0.2 s. Found by chance.
- 5b8b550e: the throwaway hubs of the tests inherited the agent's `SB_` variables (the live hub's sb-core, its socket, the agent's name). Fixed in the test helper with another list (`e2e.host_env`).
- 65607a5f: a test read the real HOME's skills.
- `tests/run_all.sh` has to `unset SB_CORE_BIN` before the Rust tests.
- `bise_home` stamps its exports (`BISE_EXPORTS_FOR` = the HOME) to notice exports that came from another HOME: the same problem, worked around.
- Rust reads environment variables in 134 places, 73 distinct names, spread over every crate (harness/src/main.rs 12, switchboard/src/daemon.rs 8, harness/src/doctor.rs 8, ...). Every feature adds one: `SB_EVERY_MIN_MS` (scheduled tasks), `BISE_HOME_WORKSPACE` (ambient-app's home workspace).
- 6586f6d2 (skills: another HOME's default path taken as an override) and 2e2ff832 are the same family.
- `tests/proc_cleanup.py` passes inside `run_all.sh` and times out alone ('t1's hub and its REPL', bg-handoff, Oct 5): `run_all.sh` sets PATH and unsets `SB_CORE_BIN`, an alone run doesn't. The likely first case for the registry.

## The result (what is true after)

1. One registry lists every bise variable with its kind: **user setting** (the user may set it: `BISE_HOME`, `BISE_ASCII`, `BISE_TERM_TITLE`...), **internal** (bise sets it for a child: `SB_SOCKET`, `SB_AGENT`, `BEND_WORKDIR`...) or **test setting** (`SB_EVERY_MIN_MS`...).
2. One function builds a child's environment for each kind of child (hub, sb-core, REPL). It starts from the parent's environment minus every internal and test variable, then sets the internal ones that child needs. A child never inherits an internal variable. The user's own environment (PATH, proxies, locale, keys) still passes.
3. A unit test fails when the code reads a bise variable that is not in the registry.
4. `e2e.host_env()`, the `env_remove` lists and the `unset` in `run_all.sh` are gone, replaced by the registry.

## No-regression check (anyone can run it)

- `tests/run_all.sh` green: the e2e tests, the tmux tests, `core_restart.py`, `idle_exit_e2e.py`, `repl_bash_env.py`, `bins_path.py`, `home_migrate.py`, PROOF.
- New unit tests: for each kind of child, a parent environment full of junk internal variables (`SB_CORE_BIN=/nonexistent`, `SB_AGENT=x`, `BEND_MODEL=junk`...) and user settings gives a child environment that has none of the junk and every user setting.
- New e2e: start a hub from a shell with `SB_CORE_BIN`, `SB_SOCKET`, `SB_AGENT` and `BISE_APP_ROOT` set to wrong values. The hub runs its own sb-core, binds its own socket, and an agent's `env` shows only the values the hub set.
- The registry test fails on a branch that adds `std::env::var("BISE_NEW_THING")` without registering it.

## Scope

- The places that spawn bise processes: `rust/switchboard/src/daemon.rs` and `daemon/repl.rs` (REPLs, sb-core), `rust/switchboard/src/client.rs` (starting the hub), `rust/harness`, the registry (in `rust/home`, next to `Home::exports`), `tests/e2e.py`.
- Hub work: it lands straight on main after the architect's review and the full gate (the user, no feature branch for tech-debt fixes), and must merge cleanly into the ambient-app branch.

## Out of scope

- Renaming variables or changing what they mean.
- The Bend runtime's own reads (it keeps reading what the hub sets).
- Replacing environment variables with files.
