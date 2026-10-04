# bise architecture principles

The architect agent keeps this file and the page `architecture` in step.
Read at main a868c748 and ambient-app c110246f. Line numbers can move:
search for the quoted code. Read it before you write or change code in this
repo (the `bise-implement` skill sends you here).

## The map in one screen

| process | code | owns | talks to |
|---|---|---|---|
| TUI (`bise`) | `rust/tui` | drafts, view state | `hub.sock`, JSON lines |
| hub (`bise sbd`) | `rust/switchboard/src/daemon.rs` + `daemon/` (the shell) | journal, transcripts, REPL supervision | sb-core (TCP), REPLs (TCP + files), clients |
| sb-core | `bend/hub/*.bend` | the hub's decisions and its durable state | the hub only: one JSON line in, one out |
| agent REPL | `bend/runtime`, `bend/core` | the session, provider calls, the bash tool | the hub (say/steer), the provider, MCP |
| `sb` CLI | `rust/switchboard/src/cli.rs` (busybox link) | nothing | `hub.sock`, one request, one reply |
| computer-use broker | `rust/computer-use` | browser links, `state.json` | `run/computer-use.sock`, Chrome host, helper app |
| bend-jsrt | `rust/jsrt` | the `run_typescript` sandbox | the REPL |
| ambient app (branch) | `apps/ambient/{mac,web,kit}`, `rust/tui/src/ambient/` | the capsule's UI state | `bise ambient-core` → `hub.sock`; the page server |

Crates: `home` (every path) ← `session`, `plugins`, `images` ← `catalog` ←
`switchboard`, `tui` ← `harness` (the `bise` binary). `tui` does not depend on
`switchboard`: it speaks the socket only.

### The seams

- **Rust ↔ sb-core** (`switchboard/src/core.rs` ↔ `bend/hub/main.bend`): one
  JSON input per line; sb-core answers `{fx, dirty}`, or `{need}` for a git
  query (the hub runs it through `Env` and sends the input again with the
  answer). The laws in `bend/LAWS.bend` cover it; `PROOF.bend` runs in the
  gate.
- **hub ↔ REPL**: `say`/`interrupt` on TCP, steer and interrupt files; the
  events come back through `wire.log` + `wire.offset`; `repl.json` lets a new
  hub adopt a running REPL.
- **clients ↔ hub**: `hub.sock` (its path from `bise_home::socket`),
  `{op: hello}` then JSON lines; the agents' `sb` uses the same socket.
- **Rust ↔ Bend runtime**: environment variables (`BEND_WORKDIR`,
  `BEND_EXTRA_PROMPT`, `BEND_CONTEXT_FILE`, `BISE_MODELS_FILE`,
  `Home::exports`) and the plugins' loopback HTTP bridge
  (`plugins/src/bridge.rs`).

### Where state lives

| state | where | source of truth |
|---|---|---|
| agents, messages, cards, waits | sb-core, rebuilt from `hubs/<ws>/journal.jsonl` | the journal (law `hub_run_replays`); `core::Hub.st` is a read-only mirror for the views |
| runtime view state (activity, places, PRs, roles, models) | `core::Hub` fields, Rust only | Rust; lost on restart by design |
| an agent's thread | `agents/<dir>/transcript.log` (append-only, stable positions) | the transcript |
| an agent's model context | the REPL's session and checkpoint | the REPL |
| artifacts, pages, features, versions | `hubs/<ws>/{artifacts,pages,features.json,versions.json}` | files the shell owns |
| user config, keys, plugins | `~/.bise/…` through `bise_home::Home` | files |

## Where does this go

- **It decides who gets woken, told, queued, or what a message/card/wait
  becomes** → sb-core (`bend/hub/core.bend`), with a law in `LAWS.bend` when
  it changes an invariant. Not in `daemon.rs`, not in a Rust module beside the
  core.
- **It runs a process, touches git, a file, a socket or the network** → the
  shell (`daemon.rs` / `daemon/*`, or `Env` when sb-core needs the answer).
  Put the pure part (parsing, layout, the decision) in its own module and test
  it there; the shell only calls it.
- **A path under `$HOME` or the state dir** → ask `bise_home::Home` (or
  `switchboard::paths::Paths`). Never join `$HOME` yourself. Sockets:
  `bise_home::socket`.
- **A new journal line** → sb-core's, through an effect. A Rust-only line
  (like today's `pr_*` and `every_*`) needs the architect's yes first.
- **Something the TUI must know** → a field of a typed event on the socket,
  never a phrase the TUI finds in feed text.
- **View-only state in the TUI** → `rust/tui`; pure layout in its own module
  with unit tests (`topedge.rs`, `termtitle.rs`), drawing in `ui.rs`/`render.rs`.
- **A new hub feature's runtime state** → a struct in that feature's module,
  held by `core::Hub` as one field. Not seven new fields on the hub.
- **A test** → on fake data: `tests/fake_provider.py`, a throwaway hub, tmux
  for the TUI. Never the user's real accounts or real hub.

## The principles

Each with a good and a bad example from this repo.

### 1. Decide in pure code, act at the edge

Inputs come in as arguments, never read from inside.

- Good: `tui/src/termtitle.rs`: `text()` is pure, the debounce is
  `Title::next`, the bytes are written only in `tick()`.
- Bad: `every.rs` (ambient-app) says "Pure" but `min_ms()` reads
  `SB_EVERY_MIN_MS` from the environment.

### 2. One owner per piece of state

Before you add a field, name who owns it.

- Good: `bise_home::Home` owns every path ("No other code joins $HOME with a
  state path"); `bise_home::socket` gives every client the same socket path.
- Bad: `Hub::replay` (core.rs, "the hub's own lines") sends `pr_*` and, on
  ambient-app, `every_*` journal lines around sb-core: the journal has two
  readers.

### 3. Hub decisions go in sb-core, with a law

Who gets woken, told or queued is a decision.

- Good: `stop_leaves_no_waiter`, `delivered_once` in `bend/LAWS.bend`,
  checked by `PROOF.bend` in every gate.
- Bad: timer wakes in `every.rs` ("a wake while busy waits, never stacked",
  retried after a minute): sb-core's queued-message logic written again in
  Rust, with no law.

### 4. Structure across a seam, text only for people

- Good: sb-core's protocol: one JSON line in, `{fx}` or `{need}` out.
- Bad: the TUI finds cards in feed text:
  `line.starts_with("sb card : ")` (`tui/src/sb.rs`, `ingest_for`), and
  `strip_prefix("sb route : ")` next to it. A wording change in the hub breaks
  the inbox silently.

### 5. One job per function, files under 1,000 lines

Split before you add, not after.

- Good: the `bise` binary's `main` went from 494 lines to 5
  (`harness/src/main.rs`).
- Bad: `daemon::run` was 411 lines when `docs/tech-debt.md` measured it, 544
  now, 573 on ambient-app; `daemon.rs` is 3,097 lines (4,492 on ambient-app).

### 6. A lesson becomes a check

The second time you write a rule down, write a test or a gate step instead.

- Good: `paths::tests::ids_match_the_python_copies` pins the Python copies of
  the workspace id; `sb land` refuses another agent's file, after b4c2d14
  undid two commits; `tests/sleep_check.py` (51c92fd3, in `run_all.sh` and
  `gate.sh quick`) refuses a new blind `time.sleep` in `tests/`, after the
  same flaky-wait fix was made test by test (docs/issues/10-tests-wait.md).
- Bad: a2138027 widened `repl_bash_env`'s margins instead of asking why the
  bash tool's 6 s window lasted 8 s: the window was counted in loop turns
  (`bg_poll_iters` in `bend/runtime/bash-pure.bend`), a clock that runs slow
  under load, and the test stayed red until bg-handoff measured it.

### 7. Prove it on the real thing, with fake data

- Good: `tests/long_socket_e2e.py` runs a real hub with a 157-byte socket
  path; `tests/core_restart.py` kills sb-core with -9 under a live hub.
- Bad: `docs/bend-laws-report.md`: the last e2e pass "date d'avant les lois
  L2 à L4 : elle n'a pas été relancée".

### 8. Move the callers, then delete the old way, in one wave

- Good: BISE-113 deleted the single-agent TUI instead of keeping it beside the
  new one.
- Bad: the `bend-harness` link "kept for one release"
  (`harness/src/main.rs:2`, `scripts/versions.sh`) is still there many
  releases later.

### 9. The doc lives next to the code

A module's `//!` header says what it owns and what it does not. Change it in
the same commit as the code.

- Good: the headers of `switchboard/src/lib.rs` and `daemon.rs`: the layout,
  the threads, who owns what.
- Bad: `docs/IMPLEMENTATION.md` says "needs_approval n'existe pas" next to
  7,645 lines of `switchboard/src/approvals/`.

## Changes approved in principle

Scheduled by main after the coming feature releases; slice by slice. Until a
change lands, do not make it worse: no new journal line outside sb-core, no
new feed-text parsing in the TUI, no growth of `daemon::run`.

1. One owner for the journal: timers and PR news go through sb-core as
   inputs; Rust-only lines only for view state, behind one trait.
2. (decided: no.) main stays the terminal product: ambient's hub pieces
   (pages, the page server, keeps, card_link) stay on ambient-app until the
   user decides to merge ambient. Only scheduled tasks (`sb every`,
   `/scheduled`) come to main. The drift between main and ambient-app is a
   known, accepted cost: do not port ambient pieces to main without his word.
3. Split `daemon::run` into one function per `Msg` kind in `daemon/`, on
   main; ambient-lead takes the split into ambient-app at its next merge
   from main.
4. A typed client protocol shared by the hub, the TUI and the ambient core.
5. Group `core::Hub`'s runtime fields by feature, in their modules.
6. The 1,000-line rule as a gate check, with a shrinking exceptions list.
7. Delete the compatibility left "for one release".
8. `docs/IMPLEMENTATION.md` becomes a short English index of the module
   headers, or goes.
