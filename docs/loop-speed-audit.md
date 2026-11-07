# The feedback-loop audit

How to find, from the agents' own traces, where the time goes between
"change written" and "landed", and whether the gates stay fast and useful.
Rerun it every day (a timer: `sb every day 08:00 "loop audit" --to main`),
and after any change to the gates, heavy-run or the tests' list. The first
run and its findings: docs/loop-speed.md §10 and the loop-speed-4 page.

## Run it

```
python3 scripts/loop_audit.py --since 24h [--json $TMPDIR/audit.json]
```

Read only, ~30 s (it reads every session written in the window). It prints
the health checks, then the sections below. `--since 8h` for one night,
`--hub <id>` for another hub than the newest. `--self-test` checks its
parsing on synthetic traces.

## Where the numbers come from

| source | what it gives |
|---|---|
| `~/.bise/sessions/*/events.jsonl` | each agent's turns and tool calls with their times; edits (`edit`, `write_file`) and lands (`sb land` that answered "landed") |
| `~/.bise/hubs/<hub>/journal.jsonl` | the wakes: `wake_set` (kind job, file, bg, pid), `wake_end` (hit, max, stopped) |
| `~/.bise/gate/logs/*.log` | the gate runs: gate.sh's own log of each quick/full/rerun, kept 7 days. First line `# gate <mode> agent=<a> sha=<HEAD> start=<epoch> root=<repo>`, last `# gate end rc=<n> secs=<n>`; between, `GATE quick/full/rerun GREEN (Ns)` or `FAILED`, each step and test line, `got the gate after Ns`. A run red before its GATE line counts by its rc |
| `~/.bise/run/**/*.log` | the nightly's (`run/nightly/<date>.log`) and the release gates' logs, same lines: only those older than the first gate log (since then their gate.sh run has its own) |
| `~/.bise/gate/timings.tsv` | the tests: run_all.sh's history, one line per e2e/tmux test run (time, commit, test, rc, wall s, CPU s, load, who ran it: `gate-<mode>` from gate.sh, else `adhoc`). A gate log's test lines only for a test it lacks (quick's steps, PROOF) |

One source per fact: the agents' own copies of a gate's output (their
`$TMPDIR`, deleted at the task's drop) are not read. Runs before the first
gate log are lost; the report says so (`gate logs start <date>`).
| `~/.bise/run/heavy.log` | heavy-run: when each job was submitted and when it got its slot (a line), when it ended and its rc (a second line, since flow-time) |
| git (main's history and reflog) | when main first held each commit; a task's commits are the ones whose subject starts like its `sb land "<message>"` (scripts/flow_time.py) |

## What it computes

- **change written → landed** ("cycles"): from the first edit after a land
  to the next land, per agent. The time is cut into: `model` (in a turn, no
  tool running), `tool:<class>` (bash calls classed by their command:
  `gate`, `cargo`, `job-submit`, `wait-poll`, `test-rerun`, `land`,
  `shell`...), and the time between turns, named by what woke the next
  turn: `wait:job` (a wake on a job, a file or a background command),
  `wait:timer` (an `sb every` timer), `wait:agent` (another agent's
  message), `wait:user`.
- **spawn -> on main -> archived** (flow-time, Gabriel's metric: « le
  temps entre le lancement d'un agent et le code mergé sur main et l'agent
  archivé »): per task archived in the window, spawn -> first commit ->
  its code on main -> archived, p50 and p90, and where the hours went: each
  second of spawn -> on main counts once, in the first that holds it (its
  heavy job queued, by kind; running; the agent in a turn; the gap before
  its next turn, named by the message that woke it: the user's decision,
  a peer, main, a provider outage, a freeze, its own timer, a job), then
  the drop wait. Tasks with no commit on main are counted apart.
  `scripts/flow_time.py` (its self-test: `python3 scripts/flow_time.py`).
- **gates**: runs, green share, green time p50/p90, the wait for the
  full-gate lock, each quick step's p50, and the red runs by what was red
  (a test red even alone, a step, a setup; else the rc). A run
  on an agent's scratch tree (`agents/<a>/tmp/`, an experiment such as a
  newer Bend) is left out and counted on its own line.
- **tests**: wall p50/p90 and CPU p50 per test, how many runs it was red
  in. Runs outside a gate (`adhoc`: an agent's own loop, often without its
  binaries) are left out and counted on their own line.
- **waits**: sb wake watches by kind, the ones that ran to `--max` (a job
  that died without writing its file), heavy-run's slot waits.

## Health checks (the script prints ok / SLOW)

| check | limit | why |
|---|---|---|
| quick gate p50 | 60 s | iterate in the foreground, under the bash tool's 30 s window when warm |
| full gate p50, green | 10 min | one per task |
| full-gate lock wait p50 | 5 min | |
| heavy-run slot wait p50 | 5 min | |
| full gates red | 20 % | a red gate for nothing costs a rerun; above that, fix flakes first |
| wakes that ran to --max | 0 | a dead job nobody noticed |
| vitest-related misses | 0 | quick's desktop_check runs only the vitest files related to the change (tests/gate_plan.py drel, `<gate dir>/vitest-related.jsonl`); a file red in a full gate that a green related quick before it skipped is a suspect miss: one, and quick goes back to the whole `pnpm check` (review-gates m_33177) |
| spawn -> on main p50 | 3 h | Gabriel's metric (flow-time): 4.8 h on the 111 tasks archived 16-20 oct |

## How to read it, what to do

1. `wait:job` the biggest share: the gates are the loop. Look at the gate
   section: a quick gate far over its doc means cold targets (task
   worktrees without the seed clone) or load; a full gate's lock or slot
   wait over 5 min means too many full gates for one machine.
2. Full gates mostly red: take the failing tests list. A test red in more
   than 1 of 10 runs and green alone is a flake: fix its cause (the
   loop-speed.md tables say how the earlier ones were found), never raise
   its timeout blindly. `gate.sh rerun` keeps the agents moving meanwhile.
3. A test whose p50 grew: compare with the previous audit's slowest tests;
   timings.tsv has the commit of each run, so the change that made it slow
   is between two lines.
4. `wait:agent` large: not the gate's fault (reviews, a lander, main);
   say so to main.
5. Wakes at `--max`: an agent waited on a file a dead job never wrote.
   Watch the job (`sb wake --on-job <label>`), not only its file. A wake
   whose heavy-run job was still in the queue at its `--max` (heavy.log:
   submitted from the agent's tree, not started) is listed apart and not
   counted: the job was alive; the agent should have run it as a
   background command (loop-speed.md §10, heavy-run).

Keep the thresholds here and in `health()` of the script the same.
