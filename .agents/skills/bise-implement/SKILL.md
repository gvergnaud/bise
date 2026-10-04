---
name: bise-implement
description: Read before writing or changing any code in the bise repo (Rust crates, Bend hub and runtime, TUI, tests, scripts, the ambient app). Says what to read before touching each area, the hard rules on where code and state go, and when to ask the architect agent. Use for every implementation task, bug fix, refactor or feature in this repo.
---

# bise-implement

Before you write code here, read `docs/architecture-principles.md`: the
map, the seams, "where does this go", and the 9 principles with an example of
each from this repo. It is short. The architect agent keeps it current.

## Before you touch an area, read

- the hub's decisions (messages, cards, waits, delivery): `bend/hub/core.bend`,
  `bend/LAWS.bend`, `docs/bend-laws-report.md`;
- the hub shell: the `//!` headers of `rust/switchboard/src/lib.rs`,
  `daemon.rs`, `core.rs`;
- the TUI: `rust/tui/src/lib.rs`, `sb.rs`, and the module you change;
- paths, sockets, state files: `rust/home/src/lib.rs`;
- the agent runtime: `bend/runtime/repl-core.bend`, `repl-live.bend`;
- tests and gates: `tests/gate.sh`'s header, `docs/loop-speed.md`;
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
6. No change pushes a file over 1,000 lines, or makes `daemon::run` longer:
   split first.
7. Replace an old way and delete it in the same change; no "kept for one
   release".
8. Update the module's `//!` header in the same commit when what it owns
   changes.
9. Tests on fake data (fake provider, throwaway hub, tmux); never the user's
   real accounts or hub.

## Unsure? Ask before writing it

Unsure where something belongs, whether a change breaks a seam, or whether a
rule above applies: `sb ask architect "<the change, the files, your question>"`
before writing it. No answer in time: end your turn, the reply wakes you.

No `architect` agent in `sb list`: follow `docs/architecture-principles.md`,
and say in your report which rule you followed and what you were unsure of.

If you must break a rule, say so in the commit message and the report, with
the reason.
