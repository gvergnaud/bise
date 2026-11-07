# Parallel Bend in bise: the rules, measured

Read this before you write a def in bend/ that walks a list or a tree.
Models were never trained on Bend's parallelism: by habit they write list
folds and accumulators, which run on one core. The same rules with their
full bench code, for any Bend project: `~/.agents/skills/bend/parallelism.md`.

Numbers: Bend 2.0.39, native binaries, `./f +RTS --threads N -RTS`, min of
3 runs under heavy-run `--low`, Gabriel's Mac (12 cores, 8 performance) at
a load of 20-90 from the other agents (2026-11-07, task bend-parallel).
Wall speedups are lower bounds; CPU (user) time is the steady number.
"1 / 8" = wall time at 1 thread / at 8 threads.

## The rules

1. **Only a parallel let forks**: `a b = f(x) g(y)`. The same calls on two
   lines run one after the other. Nothing else forks: not a map, a fold,
   nor `IO.fork` (E1, E5).
2. **Only the native build runs it in parallel**: sb-core and repl-live are
   built with `bend -o` (scripts/bins.sh), so they do. The JS target, the
   `.mjs` modules the desktop imports and pages run sequentially (E7).
3. **Two equal halves.** The scheduler hands each task to a core once and
   never moves it. Recursion that halves itself (a range, a balanced tree)
   is the best shape; a parallel let on a list's spine (`h` vs `t`) gives
   nothing; a 1/8 vs 7/8 split gives 2x where halves give 6x (E2).
4. **A list: split it a few levels deep, fold each part.** 6 levels of
   `List.take`/`List.drop` (64 parts) then a plain fold below
   (vendor/json's PItems does this). Each level copies the spine (E2).
5. **Real work per item, or don't split.** 4 steps per item: splitting
   cost 11x the CPU of the fold and ran 4.7x slower (E3). Forking a
   recursion that copies nothing is cheap: 67M forks cost no measurable
   CPU (E4). Fork recursion freely; never build or split data just to fork.
6. **What blocks it**: an accumulator threaded through the walk (combine
   the halves' results with an associative op instead); an early exit (a
   parallel search runs both halves to the end: E6, and bend-perf's
   pattern 1 first); effects (`IO.fork` interleaves on one event loop: for
   waits, never CPU: E5); a sequential envelope around the parallel core
   (building the input, joining the output: B1, and bp-boot in the audits).
7. **Check it**: time the compiled binary at `--threads 1` and `--threads
   8`; CPU / wall near 1 = one core busy = no fork. sb-core: `sample` must
   show several worker threads in the def (1.1 cores = no gain).
8. **A hub step stays sequential** (each step reads the state the last one
   wrote): parallel fits pure bulk work inside one step or one batch.

## The language examples (code in ~/.agents/skills/bend/parallelism.md)

| | shape | 1 / 8 threads |
|---|---|---|
| E1 | 65,536 leaves x 20k steps, parallel let | 2.86 / 0.91 s (2.31 / 0.30 at a lower load) |
| E1 | the same, two lets | 2.47 / 4.08 s (no fork) |
| E2 | 65,536 items x 10k steps: fold, spine fork | 1.66 / 1.96, 1.82 / 1.88 s |
| E2 | halves 6 levels, lopsided 1/8, range halving | 2.92 / 0.49, 5.04 / 2.55, 2.16 / 0.27 s |
| E3 | 8M items x 4 steps: fold vs halves | 0.13 / 0.17 vs 1.56 / 0.80 s (CPU 0.06 vs 0.66) |
| E4 | 64M 4-step leaves: two lets vs parallel let | 0.77 / 0.79 vs 0.85 / 0.14 s, same CPU |
| E5 | two 2 s computations: IO.fork, parallel let | 2.98 / 2.75 (1.0 core), 3.14 / 2.05 s (1.7 cores) |
| E6 | search, hit at 1,000 of 65,536: stops early vs parallel | 0.03 / 0.02 vs 1.65 / 0.24 s |
| E6 | hit at 65,000 | 1.52 / 1.65 vs 2.13 / 0.25 s |
| E7 | the same program, native vs `-o f.js` on node | 0.20 / 0.036 s vs 0.35 s |

## bise's own code

Bench copies of bise's code, not bise itself (the kit: artifact
`bend-parallel bench kit`, task bend-parallel).

### B1. vendor/json stringify of a big array (view_all's agent rows)

65,536 rows of 4 fields (a 400-char brief), 28.6 MB of JSON. Before =
`arr_tasks` with 0 levels (item by item), after = today's PItems, 6 levels.

| | 1 / 8 threads | CPU |
|---|---|---|
| stringify + join + length, before | 1.59 / 1.40 s | 0.71-0.77 s |
| the same, after (PItems) | 1.57 / 1.27 s | 0.90-0.95 s |
| stringify alone (chunks, no join), before | 2.41 / 3.81 s | 0.78 s |
| stringify alone, after | 4.29 / 2.53 s | 1.00 s |

Rows built and sized alone: 0.03 s. The parallel print buys 10-35 % of
wall for 25-35 % more CPU (bp-view measured 1.92 -> 1.5 s on view_all).
Little, because each item is small work against what stays serial: the
32 MB join (`join_chunks_loop`) after it, and `halves_join`'s
`List.append` of the chunk lists at each level (not measured apart).

### B2. Parsing a batch of journal lines (the replay)

65,536 lines of ~330 bytes, already split. One by one (`parse_all`, the
replay's shape): 0.65 / 0.59 s, CPU 0.31-0.38 s. In halves 6 levels deep:
1.24 / 0.32 s, CPU 0.54-0.58 s. Building the lines alone: 0.05-0.10 s.
1.85x at 8 threads under load, for 50 % more CPU. It pays only on lines
that are already separate strings: bp-boot (Audits) found the batch
arriving as one JSON string cost more to decode than the whole replay.

### B3. queued_in's shape: a cheap test per message

262,144 messages, a Bool match then a short string compare: the fold
0.051 / 0.063 s (CPU 0.03 s); halves 6 levels 0.12 / 0.071 s (CPU 0.06-0.07
s). Never faster, twice the CPU. The fix for a cheap walk is to walk less
(bp-lazy: the cheap test first, stop at the hit: Tick 41 -> 10.6 ms), not
to fork it.
