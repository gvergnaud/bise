# 15 · a hub's boot replays the whole journal

Status: open. Found by `hub-boot-hang` (2026-10-07). Label: hub, boot.

## The problem

Each hub start sends the whole journal to sb-core (`Hub::replay`,
`replay_many` batches). The time grows with the journal, and more than
linearly. On the harness workspace (37.7k events, 16 MB), on 2026-10-07:

- idle machine: ~10 s (hub.log, pid 62062: `journal read` 08:46:48.2,
  `journal replayed` 08:46:58.7);
- load 15-40 (cargo gates, Spotlight): more than 25 s. Before
  `hub-boot-hang`, the switcher killed a hub after 20 s with no new
  hub.log line. A switch to 3981c229 failed for this reason, then its
  rollback failed too, and no hub ran for 68 min.

`hub-boot-hang` made the boot report its progress (`journal replay n/N
events`, crate::boot), made the switcher wait while it moves, and made a
rollback leave a slow hub booting. The boot itself is still as long.

## Measurements (sb-core f22007bd, a copy of the journal, load ~12-22)

Time to replay, by events per `replay_many` line: 5000 → 22.7 s, 500 → 17.0 s,
100 → 11.6 s, 50 → 13.8 s, 20 → 12.7 s. REPLAY_BATCH is now 100.

Seconds per 5k events grow along the replay (batch 500): 0.39, 0.95,
1.54, 2.10, 2.27, 3.75, 4.0. The journal's bytes per 5k events are flat
(2.1-2.9 MB).

Most of it is not `apply`. The same lines sent as `{"t":"ping","evs":[..]}`
(sb-core parses the line and does nothing) take 15.5 s out of 17. The
same 5k events sent 7 times also get slower: 0.32 s, then up to 2.04 s
per 5k. So the slowdown grows with the work the sb-core process has
already done, not with the state. It looks like something in the runtime
(heap, GC, the socket line reader). Not found yet. Architect (m_10760):
it looks like the Bend allocator aging that repl-cpu-3 found in
repl-live (m_9899, the age.bend repro). sb-core is a long-lived Bend
process too, so the allocator fix would help both the boot and sb-core's
steady CPU. A snapshot alone does not cure the aging.

## What to do (not in hub-boot-hang)

1. Find why the parse in a long-running sb-core gets slower (the runtime
   or the TCP line reader). It may be the biggest win, and it would also
   speed up a hub after its boot.
2. A state snapshot: sb-core writes its durable state every N events
   (`snapshot` line, owned by sb-core: architecture rule 2), and a boot
   replays only the events after the last snapshot. Open questions:
   - old_core_replay: a rollback to an older sb-core that cannot read a
     newer snapshot must replay the whole journal (a version field on the
     snapshot, the full replay as the fallback);
   - the laws: a replay from a snapshot must give the same state as a full
     replay (a LAWS.bend law, the PROOF over sampled journals);
   - the Rust mirror (`Hub::view_all`) and the hub's own lines
     (`forge::is_pr_line`) still read the whole journal, or get their own
     snapshot;
   - `Revive` (BISE-292) restarts sb-core on the journal: from the
     snapshot too.
