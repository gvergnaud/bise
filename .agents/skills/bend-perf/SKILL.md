---
name: bend-perf
description: Audit and speed up bise's Bend code (sb-core in bend/hub, the agent runtime in bend/runtime) without changing what it does - measure first, find the costly patterns (eager picks, full list walks, string appends, unused fork/join), fix them with the same results and laws. Use when asked to make the Bend code faster, to audit its performance or its use of parallelism, or before optimizing any .bend file.
---

# bend-perf

How to find and fix slow Bend in bise. Same results, same laws: a perf change
never changes what a step answers or writes.

## 0. Read the language first

Run `bend guide` (or `~/.bend/bin/bend guide`) and read "Recursion",
"Parallelism", "IO and Concurrency" and "Under the Hood". Look up any Base def
with `bend base <name>` before relying on how it evaluates. The facts this
audit rests on:

- **Bend is strict.** Every argument of a call is computed before the call.
  `Bool.pick(c, a, b)`, `a && b`, `a || b` (`Bool.and`/`Bool.or`) are plain
  defs, so BOTH sides are computed. Only a `match` runs one branch.
- **A `match` inspects a parameter only.** To branch lazily on a computed
  Bool, pass it to a helper def that matches on its parameter.
- **Strings are cons lists of chars** (`SCon{head, tail}`): `a ++ b` costs
  the length of `a`. Building output by appending is quadratic; build a list
  of chunks and join once (vendor/json's join_chunks).
- **Parallelism: read `parallelism.md` (next to this file) before you write
  a def that walks a list or a tree.** Its rules, each measured: only a
  parallel let `a b = f(x) g(y)` forks (two lets on two lines never do);
  only the native build runs it in parallel (sb-core, repl-live; not the
  JS target); two halves of equal cost (a lopsided split caps the gain);
  a list split a few levels deep then folded, only when each item is real
  work (a parse, a stringify: B2 1.85x; a cheap test: B3 never faster,
  twice the CPU); an accumulator, an early exit and a serial envelope
  block it; check with `--threads 1` vs `8` and CPU / wall.
- **`IO.fork`/`IO.join`** run effects concurrently on the event loop, one
  computation at a time: for waits (network, processes), not for CPU
  (measured: two forked 2 s computations used 1.0 core).
- **A hub step is sequential** (each step reads the state the previous one
  wrote). Parallelism fits only pure bulk work inside one step or one batch.

## 1. Measure (never optimise unmeasured)

sb-core on a copy of a real journal (read-only, its own sb-core):

    B=$(scripts/bins.sh path sb-core)
    /usr/bin/lockf -k ~/.bise/run/heavy.lock nice -n 10 \
      python3 scripts/core_profile.py "$B" ~/.bise/hubs/<hub>/journal.jsonl --out "$TMPDIR"

It prints the boot replay time, a Tick, a `say`, `view_all`, and writes three
`sample` files. In each, the "Sort by top of stack" list names the hot defs as
`WL_FID_<FILE>_<DEF>` (`WL_FID_MODEL_QUEUED_IN` = model.bend `queued_in`);
`rfc_wrap`, `span_fade`, `term_drop`, `heap_alloc_miss` are the runtime's
memory work (many allocations: usually string building).

Under load, wall time is noise: compare **retired instructions and cycles**
(`/usr/bin/time -l <sb-core>` around the run, or a driver that reads them),
two runs each, base vs change. And read the sample for the runtime's own
memory work (`term_drop`, `span_fade`, `rfc_wrap`, `heap_alloc_miss`): in
bise's Bend the cost is usually allocating and freeing (a buffer appended
and rescanned, a parsed value dropped, a unary `Nat` fuel built then
thrown away), more than the compares themselves.

The Rust side of a step: `SB_BENCH_JOURNAL=<copy> cargo test -p switchboard
bench_step -- --ignored --nocapture` (sb-core vs rust split per step).

The agent runtime (repl-live): `ps -o time` against its age, then `sample
<pid> 5`. If it sits in poll/select, it waits on the network: nothing to win.

## 2. The patterns to look for (cheapest fix first)

1. **Eager pick in a search**: `Bool.pick(c, Some{h}, find(t, k))` or
   `eq(h) || in(t)`: the recursion runs to the end of the list even after a
   hit. Fix: a helper that matches on the Bool and recurses only in the
   False branch. Find them: a def whose own name appears inside a
   `Bool.pick(`, `&&` or `||` on the same line. tests/pick_gate.py already
   refuses the exponential case (the def called twice).
2. **Expensive test first**: `str_in(names, to) && is_queued(st)` compares
   strings for every message, queued or not. Match the cheap test first.
   In a hot loop use a real helper that matches, not a thunk
   (`LZ.and(a, u => b)`): a closure and a cons per element cost more than
   they save (bp-lazy: thunks gave 41 -> 28 ms, the match 41 -> 10.6 ms).
3. **Full walks per step** over everything (all messages, all agents,
   archived included): keep the hot set apart (issue 21's split), or index it.
4. **Appends in a loop**: `acc ++ piece` or `xs <> [x]`: chunks, then one join;
   cons then one reverse.
5. **Pure bulk work done one by one** where the items are independent and
   costly (parsing a batch of journal events, stringifying the view's agent
   rows): split in balanced halves and use a parallel let. Before you
   fork, measure the sequential envelope around it: a batch that still
   arrives as one JSON string pays a char-by-char decode first (bp-boot,
   2026-10-17: the outer string cost more than the whole old replay), and
   check the sample shows more than one core busy (1.1 cores = no gain).
   Make the per-char work cheap (pattern 1) before you split it. How to
   shape it and what it gains: `parallelism.md`.

## 3. Change it safely

- One pattern per commit. The same answers: `old_core_replay.py` green, the
  bench_byte_equal digest equal before/after (`SB_CORE_BIN` = each build),
  ALL PROOFS CHECK (a law may unfold the def you rewrote).
- Before/after numbers from core_profile.py in the commit message, same
  journal copy, same load (under the heavy lock).
- Bend rules from `bise-implement` hold: laws for any changed decision (a perf change changes none).

## 4. Keep the record

Add a dated line to "Audits" below: what was measured, what changed, the
numbers. The next audit starts from there.

## Audits

- 2026-10-17 (architect, page bend-perf): his journal copy, 112,404 events.
  Boot replay 23-30 s (JSON parse about a third, then string appends and
  memory work); Tick 35 ms (model.bend queued_in + STRING_CMP: every message,
  delivered ones too, compared against names); say 15 ms (answer 10.8 KB);
  view_all 2.1 s for 30 MB (quote_string, string appends), at boot and
  restore only. Step answers are small (tick 138 B), not the whole view.
  34 searches in bend/hub walk the whole list (eager pick); zero parallel
  lets or IO.fork in bend/. repl-live: idle in poll (network-bound).
- 2026-10-17 (bp-boot): the boot replay, his journal copy (112,470 events),
  sb-core under `/usr/bin/time -l` (retired instructions; wall time was
  noise at load 20+). Base, batch 100: 133.3G instructions, 46.9G cycles.
  Eager picks in vendor json's classifiers made lazy (matches on the
  char): -1% (132.0G). The input path (main.bend read each recv'd chunk
  by appending to the buffer and rescanning it whole): each char read once,
  119.3G (-10%), cycles 34.7-38.5G (-18%). REPLAY_BATCH 100 -> 250: 113.7G
  (batch 500: 112.2G; cycles flat 100-500, up past 1000). Tried and
  dropped: a `replay_lines` link with the batch's lines parsed in parallel
  (the outer JSON string's decode alone cost more than the old replay:
  every quote escaped; parse_step is ~5% of the replay, the cap of any
  parallel parse); parse's fuel as a constant (-0.8%). What is left is the
  runtime's memory work: allocating and freeing each line, its parsed
  Json and each event (term_drop, span_fade, rfc_wrap in the sample).
- 2026-10-17 (bp-view, chunk 3): view_all on the same journal copy,
  back to back in one heavy slot (load ~8): 1.92 s -> 1.13-1.25 s, bytes
  identical (sha256 f170e123b550). vendor json: quote_string in one pass
  (escape_onto_str), and stringify prints a JArr of 256+ items in
  parallel halves (PItems, 6 levels) joined once: 1.92 -> 1.5 s.
  main.bend send_line: the newline as its own send, no 30 MB append:
  1.5 -> 1.2 s. Left, serial: join_chunks_loop's copy of the 30 MB
  (~0.2 s) and io_cbuf's String -> bytes (~0.17 s); next step if
  view_all matters more: answer as a list of parts, one send each.
- 2026-10-17 (bp-lazy, chunk 1): searches stop at their hit. hub/lazy.bend
  (LZ.pick, LZ.or, LZ.and: the second side a thunk `u => f(t)`) in every
  Maybe/Bool search of bend/hub and the put/set ones (the full maps,
  rename/mark, stay maps: no law says their key is unique); queued_in
  matches is_queued first (queued_in.keep), str_in only for a queued
  message. Same journal copy, back to back: Tick 41.4 -> 10.6 ms (the
  first try, LZ.and in a Bool.pick, gave 28 ms: a closure and a cons per
  message, then dropped), say ~15.5 -> ~13.4 ms, replay 21.5 -> 17.8 s;
  digest identical. pick_gate's third check refuses a strict search in
  bend/hub. Left: queued_in still walks every hot message (14k on his
  hub, term_drop is most of the 10 ms).
- 2026-10-22 (big-msg-perf): repl-live, one 2 MB `say` on the fake
  provider (/usr/bin/time -l). Base 317.6e9 instructions, 53.5 s user,
  sample: STRING_SPLIT, STRING_APPEND, term_drop under repl_conn and
  cmd_whole: repl-core.bend appended each 4096-byte recv to the
  unfinished line and split the whole line twice per recv. Now
  repl-core-pure's feed keeps the line as its pieces, splits only the
  new chunk and joins once: 25.0e9, 2.28 s user (small turn: 5.65e9,
  0.66 s), request body sha256 identical (2 MB and small).
  tests/repl_read_cost.py (run_all): x3.8 instructions for x4 the line,
  x14.8 on the old path. Left (follow-up): the rest is linear, led by
  the memory work (term_drop 213, rfc_wrap 149, span_fade 122 in the
  2 MB after-sample), then wire escape_nl and json escape; persist.bend
  read_all's chunk append is the
  same pattern on a fallback path (not measured).
- 2026-11-06 (view-all-perf): view_all on boot-speed's synthetic
  harness-sized hub (215k events, 54k messages, 1,256 agents), boot_phases
  bench, retired instructions. 58.1 MB, sb-core 29.1G, hub decode+load
  5.8G -> 10.8 MB, 9.1G, 2.2G. The message bodies were ~85 % of it:
  view_all now sends an old message that owes nothing as a cold row
  (View.cold, a 10-item array, no text or parts; view.bend ships_whole:
  hot, unsettled question, or one of the RECENT 256 newest go whole) and
  the hub asks the link's msg_bodies for a body a reader needs
  (core.rs fetch_bodies). Left: the agents, 5.1 MB of the 10.8 (archived
  ones' briefs 2.1 MB, reports 1.2 MB, files 1.2 MB).
- 2026-11-07 (bend-parallel): parallelism measured on Bend 2.0.39 native
  binaries, 1 vs 8 threads, heavy-run `--low` at load 20-90:
  `parallelism.md` (E1-E7 language rules, B1-B3 on copies of bise code).
  vendor/json's parallel print: 10-35 % of wall for 25-35 % more CPU on
  28.6 MB (serial join and per-level chunk appends left); a batch parse
  in halves 1.85x; a queued_in-shaped walk never faster forked.
- 2026-11-07 (c4b-core-step-flat): a step's cost against the archived
  agents, tests/core_step_cost.py (25 live + N archived that each ran a
  turn; retired instructions per step via proc_pid_rusage). A journal
  replay alone hides it (no runtimes): core_profile showed 0.1 ms ticks
  while the fake hub's loop waited ~46 % on sb-core. Four walks of every
  agent that ever ran: V.changed diffed the marks of every runtime not at
  rest, quadratic (tick 178M -> 0.36M at 200; land 1); rt_put appended,
  so a live runtime sat behind every archived one (rt_put to the head;
  land 2); every durable view carried St.order, every name (land 3, ~15M
  of a say at 1,000); get_agent/alias_of misses walked arch for the
  user's name and for "" (the `from` of his says) (M.never_agent, law
  no_reserved_names; land 4). At 1,000 archived: tick 4.5G (est.) ->
  0.36M, sb list's step -> 0.47M, say 27.5M -> 2.7M, flat from 200.
