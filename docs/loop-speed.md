# loop-speed: why the agents' feedback loop is slow

Measured on 2026-09-29 on this machine (M-series, 12 cores, 48 GB RAM, ~10 GB
free disk), HEAD 9b8ad22 (applied on 6bf6a86+), with other agents working at the same time
(load average 20-26). Raw logs are in /tmp/loop-speed/ while they last.

## 0. Gating rules for agents (applied)

1. Start a task with `tests/gate.sh new <name>` (from any tree; `<name>` defaults to `$SB_AGENT`): its folder `~/.bise/worktrees/<project-id>/<name>/` (BISE-230, like Codex's `~/.codex/worktrees`) holds a worktree of HEAD (`harness/`) and its own target (`target/`), an APFS clone of the warm seed for the current deps (~3-10 s, 0 bytes; the seed, `~/.bise/cache/gate-seed/<key>`, one at a time, ~7 GB of blocks the clones share, is built once per Cargo.lock/Cargo.toml/.cargo/config.toml/rustc change, ~2 min, by the first task that needs it). Run what it prints (`cd` + `export CARGO_TARGET_DIR`). End it with `gate.sh done <name>` (refused while the worktree has uncommitted changes or commits on no branch). The hub removes the folder too, at the task's /drop and, for an orphan (a task archived or unknown), at its start, never when the worktree has such work (it says so in main's thread). Never clone another task's target: they grow to 15 GB (see §8).
2. Each commit: `tests/gate.sh` (quick), in the foreground. It runs clippy -D warnings (in `$CARGO_TARGET_DIR/clippy`, next to the tests) and the tests of the changed crates and their users; no `cargo build` any more (the full gate builds the binary). A change to a `.bend` file of core/ hub/ vendor/ or LAWS/PROOF runs PROOF.bend in 4 shards in parallel; the tests run `scripts/bins.sh path sb-core`, the cache file itself (SB_CORE_BIN; on a hub/ vendor/ change it compiles next to cargo, once per source hash for every worktree and the full gate). PROOF is cached by content in `$CARGO_TARGET_DIR/gate-cache`. An sb-core gone after its build (the EDR of a company Mac deletes fresh unsigned copies at their first spawn) is one line: `sb-core vanished after it was built: an antivirus (EDR) may have removed it`. Warm: **~5 s** for a Rust change, **~11 s** for a hub change, 16 s the first time in a new worktree with a cloned target.
3. Once per task, on the last commit: `gate.sh full` (one at a time on the machine, rule 5; cargo build; ./repl-live ./repl-scripted ./sb-core put in place by `./bins.sh`, compiled when their sources changed; run_all.sh with FUZZ_RUNS=2000: every Rust test, the whole PROOF.bend, e2e and the 14 tmux tests, 4 jobs in parallel), ~55 s warm.
4. Never `sleep N; tail log`. A gate the bash tool put in the background: `gate.sh wait <its .out file or pid>` (blocks until it ends, at most 25 s, then shows the result).
5. One full gate at a time on the machine (BISE-244, §9): `gate.sh full` takes a lock; a second one prints `waiting for the gate: <agent>'s full gate runs` and sets your note to `waiting for the gate`, then runs. Do not kill it to retry, and do not run `run_all.sh` by hand to skip the queue: two full gates at once take as long as two in a row and load the machine for everyone. Quick gates never wait. Iterate with the quick gate; the full gate once, before the last commit. If a full gate fails on a tmux timing, rerun that test alone (`python3 -u tests/<t>.py`) before blaming your change.
6. `run_all.sh --serial` runs e2e/tmux one by one (debugging a flake); `SB_TEST_JOBS=n` changes the parallelism; `FUZZ_RUNS=2000` for the long fuzz run.
7. A red gate: fix forward, rerun the same gate; say in your report which gate you ran (quick/full) and its time.
8. The Bend binaries (repl-live, repl-scripted, sb-core, harness-demo) are **not in git** (BISE-114, since 2026-09-30): never commit them, even with a hub/ or runtime/ change. `./bins.sh <name>...` puts the build of your tree's sources in place, from a cache keyed by the content of the sources and shared by every worktree and versions.sh (`${XDG_STATE_HOME:-~/.local/state}/switchboard/build/cache`): a hit is a copy (~0.3 s), a miss a compile (sb-core ~15 s, a REPL 1-2 min, once per source state for everyone). run.sh, run_all.sh, gate.sh (full, and quick without a hub change) and move-live.sh call it; a fresh worktree needs nothing else. A binary the EDR ate: `./bins.sh <name>` again. A version (versions.sh) gets the RELEASE engine of its sources, `bins.sh path bend-jsrt` (same cache, BISE-133). The V8 engine goes to the runtime as BEND_JSRT_BIN (the harness finds `bend-jsrt` in its app root, else rust/jsrt/target/debug, else release); run.sh rebuilds it when rust/jsrt or rust/images is newer, and always runs `cargo build` for bend-harness (~0.2 s when nothing changed).

9. Fewer model round-trips (each one re-reads the whole context; tasks make 126-312 tool calls):
   - batch: one bash call runs everything that does not depend on an answer (`git status; rg ...; sed -n 10,60p a.rs; sed -n 200,240p b.rs`);
   - read targeted ranges (`rg -n` then `sed -n a,bp`), never a whole big file, and never the same range twice: note what you need the first time;
   - edit with one patch per file (or one script for many), then one gate; no read-back of an edit the tool confirmed;
   - message another agent only when you change an interface it uses (a function signature, a file format, a gate rule); otherwise, just rebase at commit time and fix conflicts then.
10. A commit subject is at most 72 characters; the details go in the body (`git commit -m "<subject>" -m "<body>"`). The hub lists the versions by subject: 2 KB one-line subjects made that list ~39 KB and blocked its hello.

## 1. Where the time goes

### 1.1 One gate, as the agents run it today

Fresh CARGO_TARGET_DIR (cold), default dev profile, then run_all.sh.

| step                                           | measured | note |
|------------------------------------------------|---------:|------|
| cargo build (cold: 207 units)                  |    27 s  | deps are 22 s of it (ring 11 s, syn, clang-sys, bindgen) |
| cargo test -p bend-tui --no-run (cold)         |    12 s  | |
| **cargo test -p bend-tui (run)**               | **210 s** | **one test: fuzz_random_input_never_panics = 128 s alone, 210-235 s next to the others** |
| cargo test -p switchboard (build + run)        |    11 s  | 100 tests in 2-3 s |
| cargo clippy --workspace --all-targets (cold)  |    16 s  | warm: 0.2-1.5 s |
| run_all: build + clippy (warm)                 |     1 s  | |
| **run_all: cargo test again**                  | **216 s** | **the fuzz test a second time** |
| run_all: bend PROOF.bend                       |    11 s  | |
| run_all: e2e.py                                |    17 s  | |
| run_all: 13 tmux tests, one after the other    |    73 s  | 1.4-16 s each (onboarding 16, queue 11, archived 7) |
| **total**                                      | **~10 min** | matches the agents' logs: median gate wait 10.3 min, p90 33 min |

Without the fuzz test, every other one of the 417 Rust tests runs in less
than 7.4 s (nextest: all 416 others take 8 s of wall time together).

Things that are **not** the problem (measured):

| suspect                      | measured |
|------------------------------|----------|
| incremental rebuild after a one-line edit in tui | build 1.6 s, test binary 1.7 s, clippy 1.5 s |
| linker                        | the whole relink + test binary is under 2 s |
| target size                   | 1.8 GB for build + test + clippy (not 8 GB: the shared rust/target is 8.4 GB because of 2.3 GB of incremental data, old artifacts and a release dir) |
| macOS syspolicyd first-exec scan of a new binary | 0.35-0.45 s per new binary, once |
| tmux test waits (time.sleep) | the tests poll the screen every 0.1-0.2 s; the fixed sleeps are ~1 s per test (1.1 s in archived, 2 s in undelivered, 1 s teardown) |

### 1.2 Where the agents' time goes (from their transcripts)

bise-f-feed, bise-k-keys, bise-c-cards, bise-o-onboard, bise-h-hub,
bise-quality: 15.0 h of busy time (idle time not counted).

| activity                                  | time   | share | calls |
|-------------------------------------------|-------:|------:|------:|
| model (thinking and writing between tools) | 7.7 h | 51 %  | |
| **`sleep 20-29; tail log` polling**        | **5.4 h** | **36 %** | **736 polls** |
| other shell commands (git, rg, sed, ...)  | 1.2 h  | 8 %   | 1785 |
| foreground cargo test / build / clippy    | 0.4 h  | 2 %   | 133 |
| foreground run_all, e2e, tmux, bend       | 0.3 h  | 2 %   | 260 |

- A gate is longer than the bash tool's 30 s window, so it goes to the
  background and the agent polls it: median 13.5 polls per gate, up to 60.
- Each poll is a full model turn: 25 % of all the input tokens of these
  six agents (163 M of 645 M) were spent re-reading the context to learn
  "still running", plus 69 min of model time only to decide to poll again.
- Fixed `sleep 28` also loses on average half a period (~14 s) after the
  gate has finished, and `sleep 28; sleep 28` (seen often) is > 30 s, so
  the poll itself goes to the background and needs another poll.
- Agents serialize run_all between them (ask, wait, ping), and agents'
  gates run at the same time on 12 cores: cargo test took 235 s under
  load vs 128 s for the fuzz test alone.

## 2. Causes, in order

1. **The fuzz test** (`fuzz_tests::fuzz_random_input_never_panics`):
   2000 runs x 60 random events, each drawing the whole UI through ratatui,
   in an unoptimized debug build: ~63 ms per run, 128 s. It is ~95 % of
   `cargo test -p bend-tui`, and every gate runs it twice (the per-commit
   test and again inside run_all).
2. **Dependencies built at opt-level 0**: ratatui, vt100, unicode-*, serde
   run 7x slower than optimized; this is what makes the fuzz test slow.
3. **The gate is longer than 30 s**, so it cannot run in the foreground:
   the agents poll it with fixed sleeps, one model turn per poll.
4. **run_all.sh repeats** build + test + clippy that the agent just ran,
   and runs the 13 tmux tests one by one (73 s) although each one uses its
   own tmux session and its own throwaway hub.
5. **Cold target per task**: 55 s (build 27 + test 12 + clippy 16) at the
   start of every task/gate, and 2-8 GB of disk per task on a disk with
   10 GB free (that is why tasks delete and rebuild their targets).
6. **Serialized run_all across agents**: coordination messages and waiting
   for a slot (bise-k-keys waited and held its qa capture for this study).

## 3. Improvements, ranked by gain x cost

Measured on a prototype in /tmp/loop-speed-wt (patch at the end).

| # | change | gain (measured) | cost | risk |
|---|--------|-----------------|------|------|
| 1 | **Optimize dependencies in dev**: `[profile.dev.package."*"] opt-level = 3` in rust/Cargo.toml | fuzz test 7x faster (200 runs: 12.8 s -> 1.8 s); rest of bend-tui tests 1.6 s | 3 lines | cold build +6 s (27 -> 33 s), once per target. Our own crates stay at opt-level 0: an edit still rebuilds in ~1 s; debugging our code unchanged |
| 2 | **Fuzz test: default FUZZ_RUNS 300 (was 2000)**; the full gate (or a nightly) sets FUZZ_RUNS=2000 | with #1: cargo test -p bend-tui 210 s -> 3-6 s | 1 line | fewer random sequences per commit (still 18 000 events); the fixed seed stays, so it is still reproducible. The 2000 runs still happen once per task in the full gate |
| 3 | **Foreground gate script** `tests/gate.sh` (quick = build + test + clippy; full = + FUZZ_RUNS=2000 + PROOF + e2e + tmux in parallel), and rule: run it in the foreground, no `sleep; tail` polling | quick gate warm: **4-8 s** after an edit (was ~4.5 min); no polls: -36 % of agent busy time, -25 % of input tokens | small script + rule change | a full gate that still goes past 30 s needs one wait: use `sb`-side or a `while kill -0 $pid; do sleep 1; done` capped at 25 s, never a fixed 28 s sleep |
| 4 | **Warm seed target, cloned per task with APFS copy-on-write**: keep one warm target on HEAD (/tmp/sb-seed-target); each task does `cp -cR /tmp/sb-seed-target /tmp/<task>-target` | clone 5.5 s and **0 bytes** of disk until files change; first build in a new worktree 0.2-3.7 s instead of 55 s | small; the seed is refreshed after merges (one `cargo build && cargo test --no-run && cargo clippy` on HEAD) | two tasks never share one live target dir (no lock waits, no `target/debug/bend-harness` overwritten by another task); stale seed only costs a rebuild. A single shared CARGO_TARGET_DIR for all tasks also works for deps (measured: 3.7 s build in a second worktree) but tasks then block on cargo's lock and overwrite each other's bend-harness binary that e2e.py runs |
| 5 | **run_all: don't rerun the Rust part, run the tmux tests in parallel** (xargs -P 4) | PROOF + e2e + 13 tmux: 101 s -> **28 s** on an idle machine, 39 s with other agents working | small script change + one test fix | tui_archived_tmux failed once under parallel load: it drops t1, waits for "@t1 archived", sleeps 1.1 s, drops t2 and asserts "newest first" (t2 above t1); under load the order came out wrong, so the archive order depends on wall-clock timing somewhere (to find: the hub's archive timestamp or the panel sort). Until fixed, run that one test after the parallel batch. The other 13 passed in parallel |
| 6 | Change the gating rules (main) | per commit: quick gate only (~10 s); full gate once per task (~1 min); PROOF only when a .bend file changed (already the rule); drop the run_all slot protocol once #5 makes it ~1 min | rule text only | a regression caught by tmux/e2e is found at the end of the task instead of per commit; the per-commit quick gate still covers all 417 Rust tests + clippy |
| 7 | cargo-nextest (already installed) | none on wall time once the fuzz test is short; nice per-test timings and a slow-test report (`--final-status-level slow`) | 0 | none; optional |
| 8 | sccache, mold/lld, debug = "line-tables-only" | small: deps are already reused by #4, links take < 2 s | install/config | skip for now |

Not worth it: faster linker (link < 2 s), sccache (APFS clone gives the
same reuse for free), cutting the tmux waits (they poll at 0.1-0.2 s;
the fixed sleeps total ~15 s serial and vanish in parallel).

### Tests to delete or cut

The user said to delete useless or slow-for-bad-reasons tests. Measured,
only one test is slow for a bad reason: the fuzz test running 2000 debug
runs on every commit. Keep it (it found real panics, see its regression
tests) but at 300 runs + optimized deps. The next slowest tests are
7.3 s (composer_wrap_tests::typing_across_the_wrap_keeps_every_row_and_the_cursor)
and 7.0 s (at_popup_tests::every_key_on_every_row_never_panics): exhaustive
loops, 1-2 s with #1. Nothing else is above 1.4 s. I did not find tests
worth deleting for time; the tmux suite is ~5 s per test and runs in parallel.

## 4. Result with #1-#5 (prototype, measured; the seed clone #4 is not applied yet)

| loop                                             | before  | after |
|--------------------------------------------------|--------:|------:|
| edit -> quick gate (build + all Rust tests + clippy), warm | ~4.5 min | **4-8 s** |
| new task: first quick gate (clone seed + build + test + clippy) | ~5 min | **~15 s** (clone 5.5 s + 7.6 s) |
| full gate (quick + fuzz 2000 + PROOF + e2e + 13 tmux, 4 jobs) | ~10 min | **60 s** (§5) |
| agent waiting via polls                          | 36 % of busy time | ~0 (foreground) |

## 5. Applied (commits on HEAD) and measured

- rust/Cargo.toml `[profile.dev.package."*"] opt-level = 3`; fuzz_tests.rs
  FUZZ_RUNS default 300 (FUZZ_RUNS=2000 still gives the long run; the full
  gate sets it).
- tests/gate.sh: `quick` (default), `full`, `wait`.
- run_all.sh: `cargo test --workspace` (was -p switchboard -p bend-tui:
  +24 tests of bend-plugins, bend-images, bend-harness, 1 s); PROOF, e2e
  and the tmux tests in parallel (SB_TEST_JOBS, default 4; `--serial`);
  tui_term_tmux alone after the others; tui_panel_click_tmux added (it
  existed but run_all never ran it; it passes).
- e2e.py runs `$CARGO_TARGET_DIR/debug/bend-harness` when that is set (a
  gate's own target), else rust/target as before.

Measured, warm target, one other agent working:

| gate | before | after |
|------|-------:|------:|
| quick, after a one-line edit in bend-tui | ~4.5 min | 6-9 s |
| quick, cold target (new task, no clone) | ~5 min | 56 s |
| full (`gate.sh full` = run_all.sh, FUZZ_RUNS=2000) | ~10 min | 58-69 s |

Flakes seen in parallel: tui_term_tmux (Ctrl+U leaves the composer text,
2 of 5 parallel runs; never alone) now runs alone after the batch;
tui_archived_tmux failed once with 14 jobs, never with 4. At 8 jobs the
machine is overloaded (e2e 16 s -> 46 s): keep 4.

No test was deleted. All the tests that ran before still run in the full
gate, and 25 that never ran there now do (the 24 Rust tests, tui_panel_click_tmux).

## 6. Next

1. A warm seed target cloned per task (#4): 56 s -> ~15 s for a new task.
2. Find why tui_term_tmux's Ctrl+U is lost under load, then put it back in
   the parallel batch.
3. Split e2e.py across 2-3 processes: it is the long pole of the full gate
   (23 s in parallel).

## 7. Round 2 (loop-speed-2): a quick gate in ~5 s

Measured on 2026-09-29, HEAD 59b3a03, warm private target, one-line edit,
old gate.sh vs new, interleaved (load average in brackets: other agents
were working).

| change                          | before              | after              |
|---------------------------------|--------------------:|-------------------:|
| one file in rust/tui            | 7.9-8.8 s (13); 15-19 s (26-30) | **5.1 s** (6-13); 11.3 s (25) |
| one file in rust/switchboard    | 7.1-9.7 s (13)      | **5.5-5.9 s** (13) |
| one file in hub/ (PROOF + sb-core + switchboard tests) | 27-31 s (7-9) | **11.5 s** (7) |
| PROOF.bend/LAWS/core only       | nothing checked (the gate only looked at hub/ vendor/) | 5 s |
| same hub content, gate again    | 27-31 s             | cached: like a Rust change |
| new worktree, cloned warm target | 56 s (no clone)    | 16 s |
| full gate                       | 58-69 s             | 53 s (green) |

What changed:

1. **The slow Rust tests run in parallel shards** (same work, same seeds):
   `fuzz_random_input_never_panics` is 8 tests (run i in shard i % 8; the
   filter `fuzz_random_input_never_panics` still runs them all), and
   `typing_across_the_wrap_keeps_every_row_and_the_cursor` is one test per
   width (it already made a fresh app per width). bend-tui's test run:
   3.2-5.5 s -> 1.4 s. `every_key_on_every_row_never_panics` stays whole:
   split in two it raced with the other at_popup tests on the process-wide
   recent picks (2 failures in one run).
2. **clippy runs next to the tests** in its own target dir
   (`$CARGO_TARGET_DIR/clippy`, ~300 MB, 30-40 s the first time): -2 s.
3. **No `cargo build` in quick**: clippy checks every target and the tests
   compile and link the changed crates; only a link error of the
   bend-harness binary alone is left to the full gate (-1.3 s).
4. **PROOF.bend in 4 shards** (`tests/proof_shards.py`): 629 defs, of
   which 350 laws are split; each shard has every helper and reports the
   laws of the other shards as TODOs, and passes only with exactly that
   TODO count, so a wrong proof (its Location is printed) or a new
   unproven law fails. 11 s -> 5 s. The whole PROOF.bend still runs in the
   full gate.
5. **A quick sb-core**: `bend hub/main.bend -o x.c` (2.3 s) + `cc -O1
   -fno-inline` (5 s) instead of `bend -o` (-O3: 10-16 s). -O0 crashes
   clang ("live register clobbered"). The switchboard tests run as fast
   with it. It goes to the cache, not to ./sb-core: the committed binary
   only changes in the full gate (-O3, as before).
   Dropped since: a hub change compiled sb-core twice (this -O1 copy,
   then the full gate's), and on a Mac with CrowdStrike the fresh -O1
   copy was deleted at its first spawn (30+ red switchboard tests). The
   quick gate now runs bins.sh's -O3 build in the shared cache (one
   compile per source hash; ~8 s more on a miss, next to cargo's).
6. The bend part runs in the background next to cargo; PROOF and sb-core
   results are cached by the content hash of the .bend files.

Why a hub change is not under 10 s: its floor is the C compile of the
5 MB sb-core.c (5 s at best) + the C generation (2.3 s) + the switchboard
tests that need it (1.3-3 s). Running sb-core through `bend hub/main.bend`
(no compile) makes the switchboard tests 18 s. Under load (15-30), every
number above is 1.5-2x.

Lost: none of the tests; the quick gate no longer builds the
bend-harness binary (the full gate does). Measured and dropped: nextest
(no gain once no test is long), -Og (slower than -O3 for the C).

Found on the way: `tui_images_tmux` failed on 4f1ff2d (the strip rows of
BISE-108 had the bar, so the test read them as composer text; 11d8613
fixed the product). At HEAD it passed only because the divider's flash
names the path; it now pins the strip (the file name, never the path),
and its traceback is no longer swallowed by a `sys.exit` in `finally`.
`tui_term_tmux` still fails sometimes right after the parallel batch
(Ctrl+U lost; passes alone): unchanged, runs alone.


## 8. Round 3 (loop-speed-3): a new task is green in ~13 s, and the disk stays free

Measured on 2026-09-29, HEAD c4aeb47 → 045d705, 5-6 other agents building
(load average 20-60 on 12 cores; "burners" = 8 extra busy loops, the load
of ~4 more builds).

### 8.1 Task start

| step | before | after |
|------|-------:|------:|
| target of a new task | `cp -cR` of another task's target: 15 GB apparent, 70k files (69 404 `.o`), **47 s** (load 20) | `gate.sh new <name>`: clone of the seed, 1.2 GB, 2.1k `.o`, **3 s** (load 40) |
| first quick gate (one-line edit in rust/tui) | 38 s (load 20) | 9 s (load 40) |
| **new task → first green quick gate** | **~85 s** | **13 s** |
| a seed for new deps (once per Cargo.lock/Cargo.toml/config/rustc change, by the first `gate.sh new` that needs it) | — | 72 s cold (load 30-47) |
| disk written by a first gate in the clone | ~2.5 GB | ~0.5 GB |

Why the targets were 15 GB: (1) on macOS the dev profile keeps the object
files of every crate for the debugger (split-debuginfo unpacked): 69 k
`.o` files, 1 GB, mostly the deps' (nobody debugs into ratatui); (2)
targets were cloned task to task, so each carried the incremental data
(6.3 GB) and old artifacts of all its previous owners, never pruned. With
5 of them the disk went down to 0.1 GiB free during this round and a link
failed ("linking with cc failed").

Changes:

1. `rust/Cargo.toml`: `[profile.dev.package."*"] debug = false`. The deps
   carry no debug info (their panics still have a location, our crates'
   backtraces keep full debug info). One rebuild of the deps per target
   when it lands (~60 s), then targets are ~1.2 GB instead of 8-15.
2. `gate.sh new <name>` / `gate.sh done <name>` (§0 rule 1). The seed is
   `~/.bise/cache/gate-seed/<key>` (`/tmp/sb-seed-<key>` before BISE-230:
   an `mv` onto an existing seed had nested the older one in it, 12 GB), key = hash of Cargo.lock, every Cargo.toml,
   rust/.cargo/config.toml and `rustc -vV`; it holds the test build and
   the clippy build of every crate. A missing key builds on top of the
   newest seed (only the changed deps rebuild) and replaces it: one seed
   on disk. The workspace crates recompile in each task (a new checkout's
   mtimes), incrementally: that is the 9 s.

Measured and not taken:

- **One shared CARGO_TARGET_DIR for all tasks**: unsafe, not only slow.
  Two worktrees of the same commit share the artifacts of the workspace
  crates (cargo's hash of a path package is relative to the workspace, so
  it is the same in every worktree) and cargo decides "fresh" by mtime.
  Measured: worktree B adds `compile_error!` to bend-tui, then A edits and
  builds, then B builds: **0.6 s, fresh, no error**: B's gate would have
  tested A's code. Lock contention never showed up because of that.
- **sccache**: not installed; it would copy (write) each dep's output into
  each target where the clone shares it for 0 bytes, and it cannot cache
  our crates (incremental). The seed gives the same reuse for free.
- `debug = "line-tables-only"` for our crates: not needed once the deps
  are out (a gate's clone writes ~0.5 GB); keeps full debugging of our code.

### 8.2 The tmux / e2e flakes

From the failed run_all logs of the agents (the last 70 min, 12 failures):

| test | cause | fix |
|------|-------|-----|
| tui_version_tmux (3 of 3 recent failures; failed again in the "before" run below) | not load: the agents' env (and the tmux server's, which the TUI inherits) has `SB_BUILD_DIR`; versions.sh then built in the real build dir, so "build of tree failed" never came | e2e.AGENT_VARS also drops SB_BUILD_DIR, SB_VERSIONS_DIR, SB_LAUNCH_DIR, BISE_ROLE, BISE_EXPORTS_FOR (host_env and `env -u` for the TUI) |
| tui_archived_tmux "newest first" | the archived rows sort by last report; t1 and t2 were spawned in one message, so their report order was up to the load (the 1.1 s sleep between the drops did nothing) | t2 is spawned after t1 reported; the sleep is gone |
| tui_archived, tui_tmux, tui_waits, tui_onboarding: an **empty screen** at the timeout | the TUI exited early and `sleep 30` closed the pane before the 40 s timeout: the cause was lost. 3 of them failed in the same second, at a disk-full moment | the pane stays until the test's teardown with `[switchboard exited: N]`; an empty screen prints what tmux says |
| all waits under load | a 40 s bound meant for an idle machine | `wait_until`'s bound is × the load per core (1-4, read at each poll): a passing test returns as early as before, only a broken one waits longer |
| tui_term_tmux Ctrl+C | sent 0.5 s after `sleep 100`: under load it could reach the shell before the sleep ran | waits until `pgrep` sees its own `sleep 100.<pid>` |
| home_migrate "idle hub B now in ~/.bise/hubs" | `switchboard --stop` returns before the hub exits; 0.5 s later the migration still saw B busy | stop() waits until the hub is gone as `hub_busy` sees it (no socket answers, the pid is dead), up to 30 s |
| tui_queue_tmux "No such file" for /tmp/bend-sh-*.sh | the disk-full moment (13:38) | disk (8.1) |

Full gate, same load (agents + 8 burners, load 40-60), before/after:

| | before (HEAD tests) | after |
|--|--|--|
| failures | tui_version_tmux | repl_bash_env (below) |
| e2e / PROOF / onboarding / queue | 32 / 33 / 19 / 14 s | 39 / 63 / 29 / 25 s (heavier moment) |
| without burners | 53-69 s (§7) | 97 s in the gate, green (load 15-40) |

Not fixed: **repl_bash_env** fails under heavy load (1 of 3 runs at load
60, also before this round): the first command (`sleep 3; echo job-done`,
window 2 s) comes back finished instead of handed off to slot 0, so the
third one gets slot 0 and reads its own output. Widening the margins
(window 5 s, job 10 s) did not change it, so it is not the margin: the
REPL's hand-off under load needs a look (the product, not the test).
Also seen: from this agent's shell `scripted_ts` fails ("0 session
files") and passes with the BEND_/BISE_/SB_ variables unset; no single
variable is the cause. The other agents' runs pass it.

### 8.3 What agents do differently

- Start: `gate.sh new <name>`, then the printed `cd` + `export`; end:
  `gate.sh done <name>`. Never `cp -cR` another task's target.
- The round-trip rules (§0 rule 9) and short commit subjects (rule 10).

## 9. Round 4 (gate-sched, BISE-244): the gates share the machine

The user, with 6+ tasks running: load 14 on 12 cores, two full gates at
once, rustc at 60-90 % each. Measured on 2026-09-30 with 5-6 other agents
working (their load alone 8-12), so the numbers are noisy.

| what | before | after |
|---|---:|---:|
| two full gates started at the same second (worktrees at HEAD, warm) | both done at 237 s | first 199 s, second 433 s (it waits ~3 min) |
| load1 during them (5 s samples; other agents' gates too) | avg 12.5, max 17.3 | avg 16.0, max 24.6 (an old-script full gate of another task ran at the same time) |
| one full gate's CPU (`/usr/bin/time`, its tree) | | 172 s wall, 168 s user + 49 s sys: **1.3 cores on average**, ~7 at the peak (cargo build + test), 1-2 during e2e/tmux |
| run_all's clippy in a task worktree | 99 units re-checked (~90 deps: not in the seed), 10 s | 11 units, 8 s (in `$target/clippy`, which the seed has warm) |
| a gate killed (TERM, ctrl-c) | its test hubs, sb-core, REPLs and tmux shells kept running (found alive after 11 h) | all killed at once; a `kill -9`'s leftovers killed by the next gate run |

What changed (`tests/gate.sh`, `tests/run_all.sh`):

1. **One full gate at a time on the machine**: a `mkdir` lock,
   `<gate dir>/full-<uid>.lock/owner` = `<pid> <start time> <agent>
   <root>` (macOS has no flock). A second full gate prints `waiting for the
   gate: <agent>'s full gate runs (pid, worktree)` and sets its task's note
   (`sb status working --note "waiting for the gate …"`), polls every 2 s,
   then says `got the gate after Ns`. A dead owner (pid gone, or reused: the
   start time differs) is taken over, one taker at a time
   (`<lock>.takeover`). Quick gates never wait. The cost: the second task
   waits for the first (~3 min here); the gain: the CPU peaks of two full
   gates no longer add up, and the tmux tests (timing-sensitive) do not run
   next to another full gate's.
2. **nice 10 and half the cores**: quick, full and new's seed build
   `renice` themselves to 10 (`GATE_NICE`), so everything they start runs
   below the hub, the TUIs and the agents' own work; `CARGO_BUILD_JOBS`
   defaults to half the cores (6 here; a cold build of the workspace
   crates takes 4.8 s with 6 jobs, 5.0 s with 12).
3. **One seed build per deps key**: when Cargo.lock, a Cargo.toml or rustc
   changed, the first `gate.sh new` builds the seed (`<seed>.building/pid`)
   and the others print `another task builds the seed…`, wait, then clone
   it, instead of each building its own cold target (~2 min on all cores
   each).
4. **What a run starts dies with it**: each quick/full run has its own
   TMPDIR, `<gate dir>/<pid>/` (short: the hubs' sockets live under
   it; the gate dir is `~/.bise/gate`, writable in auto's sandbox, or
   `/tmp/bise-gate-` before the home migration; an agent's `$TMPDIR` is
   too deep for a unix socket path), so the tests' workspaces are there and a test hub names it in its
   command line, even orphaned. On exit (green, red, ctrl-c, TERM, HUP) the
   run kills its descendants, every process naming its TMPDIR and their
   process groups (a hub's sb-core and REPLs), and in a task worktree the
   orphans (ppid 1) of its own `repl-live`, `repl-scripted`, `sb-core` and
   `target/debug/bise`. The tmux server is never killed (the user's
   sessions may be in it; the tests use the default socket): the test's
   shell in it is. Long steps run in the background with `wait`, so a TERM
   reaches the trap at once. A `kill -9` skips the trap: the next gate run
   kills what a dead run left (its pid gone), and removes a dead run's
   folder after a day (a red run keeps it: the kept test dirs).
5. **run_all's clippy** runs in `$CARGO_TARGET_DIR/clippy`, like the quick
   gate, so it reuses the seed.

Not done, measured: **one target dir shared by the worktrees**. Cargo's
build-dir lock makes it safe, but it saves nothing: the workspace crates'
metadata hash has the worktree's path in it, so a second worktree on the
same target recompiled all 11 of them (11 s, the same as with its own
clone of the seed), the deps are already shared by the APFS clone (0
bytes), and `target/debug/bise` (the binary the e2e/tmux tests run) would
be the last worktree's build. It would also serialize every cargo command
of every task on one lock. The seed clone stays.

What tasks do differently: iterate with the quick gate (it never waits);
one full gate, before the last commit; when it says `waiting for the
gate`, wait (`gate.sh wait <.out>`), do not kill it and do not run
`run_all.sh` by hand to jump the queue; a test that failed during a full
gate: rerun it alone before blaming the change (`tui_queue_tmux` failed
once at load 24 and passed twice alone).
