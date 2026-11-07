---
name: bise-implement
description: Read before writing or changing any code in the bise repo (Rust crates, Bend hub and runtime, TUI, tests, scripts, the ambient app). Says what to read before touching each area, the hard rules on where code and state go, and when to ask the architect agent. Use for every implementation task, bug fix, refactor or feature in this repo.
---

# bise-implement

Before you write code here, read `docs/architecture-principles.md`: the
map, the seams, "where does this go", and the 9 principles with an example of
each from this repo. It is short. The architect agent keeps it current.

## Before you touch an area, read

- the hub's decisions (messages, notifications (cards in the code), waits, delivery): `bend/hub/core.bend`,
  `bend/laws/hub.bend` (its proofs: `bend/proofs/hub.bend`, the shared
  helpers in `bend/proofs/lib/`), `docs/bend-laws-report.md`;
- the hub shell: the `//!` headers of `rust/switchboard/src/lib.rs`,
  `daemon.rs`, `core.rs`;
- the TUI: `rust/tui/src/lib.rs`, `sb.rs`, and the module you change;
- paths, sockets, state files: `rust/home/src/lib.rs`;
- the agent runtime: `bend/runtime/repl-core.bend`, `repl-live.bend`;
- any `.bend` code (bend/hub, bend/runtime, bend/vendor): the `bend` skill
  (`bend guide` first), then `.agents/skills/bend-perf/parallelism.md`
  before a def that walks a list or a tree: Bend uses every core only
  where you write a parallel let on work shaped to split, and agents write
  one-core folds by habit;
- tests and gates: `tests/gate.sh`'s header, `docs/loop-speed.md`; before
  you write or change a test, the `bise-testing` skill
  (`.agents/skills/bise-testing/SKILL.md`): the cheapest test that fails;
- builds and releases: `scripts/versions.sh`, `docs/packaging.md`;
- the area's design doc in `docs/` when there is one (`artifacts.md`,
  `subscriptions-design.md`, `idle-exit.md`, ...).

## Hard rules

1. Who gets woken, told or queued is decided in sb-core, with a law when it
   changes an invariant. The Rust shell runs effects; it does not decide.
2. No new journal line outside sb-core, no new state without a named owner.
3. Paths and sockets come from `bise_home` / `Paths`, never a joined `$HOME`.
4. Across a seam, structure: a new fact for the TUI is an event field, never
   a phrase it finds in feed text.
5. Pure part in its own module with unit tests; the shell only calls it.
6. No change makes `daemon::run` (or any loop-step function) longer: split
   first. A file is split by its jobs, never by a line count.
7. Replace an old way and delete it in the same change; no "kept for one
   release". A compatibility shim an older install truly needs is registered
   in `tests/shims.txt` with its owner and removal release, and its
   `TODO(<tag>)` on each site (`tests/shims_check.py` checks both).
8. Update the module's `//!` header in the same commit when what it owns
   changes.
9. Tests on fake data (fake provider, throwaway hub, tmux); never the user's
   real accounts or hub.

## Before sb land

- the fast checks only: `tests/gate.sh quick` GREEN on your sha (for
  apps/desktop it runs typecheck, lint, the related vitest units and the
  build: `pnpm check`'s fast half). No end-to-end before a land: no
  `pnpm test:land`, Playwright, desktop harness check, as-he-does row,
  `*_e2e.py`, `*_tmux.py` or `gate.sh full`. They run on main after the land
  (the tip gate, the nightly, a release); a red there names you in tip-red
  (docs/loop-speed.md §0 rule 11).
- heavy-run queues by rank (docs/loop-speed.md §10.6): your own script that
  wraps a land check runs as `heavy-run --land`; an experiment, a load test,
  shots or a measure as `heavy-run --low`, never `--prio`.

## Unsure? Ask before writing it

Unsure where something belongs, whether a change breaks a seam, or whether a
rule above applies: `sb ask architect "<the change, the files, your question>"`
before writing it. No answer in time: end your turn, the reply wakes you.

No `architect` agent in `sb list`: follow `docs/architecture-principles.md`,
and say in your report which rule you followed and what you were unsure of.

If you must break a rule, say so in the commit message and the report, with
the reason.
