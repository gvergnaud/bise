# 21 · the hub's hot state carries every message and every agent it ever had

Status: plan agreed by architect (m_15238), not started. Found by hub-fifo (2026-10-09). Label: hub,
perf. Follows bcca8231 (one pass per step) and hub-fifo's inbox lanes and
state throttle.

## The problem

Every step of the hub costs O(agents × messages) or O(messages), and both
only grow. On his harness hub (journal copy of 2026-10-09: 53,391 events):

- 474 agents, 450 of them archived (95 %);
- 14,658 messages, 14,656 delivered, 2 rejected; 33 settled by a reply.
  Nearly all of them can never change anything again: delivered, and no
  reply owed (or the reply came).

Yet each step walks them:

- **sb-core** (`bend/hub`): `St.msgs` is one list of `MRec` (model.bend);
  the owed-reply, queued and waiter checks filter the whole list
  (model.bend `MRec.settled`, the filter near line 749), and an agent is
  found by a walk of a list that holds the 450 archived ones too.
- **the Rust mirror** (`core::Hub.st`): `Waiting::of` walks every
  message for each snapshot (board.rs), `State::unanswered_for` does too
  (model.rs), and `Group::of` builds a roster row for each of the 474
  agents; the snapshot sends archived rows that never change.
- **the boot** replays all of it (issue 15).

bcca8231 made each of these walks happen once per step instead of once per
agent; hub-fifo made the snapshot go at most 10 times a second and put
the clients ahead of the REPL lines. The walks themselves are still as
long as the history.

## The idea

Split the state into a hot part, what a step can change or must read, and a
cold part, what is only history. The journal does not change (law
`hub_run_replays`): it stays the durable truth; only memory is split.

- A message is **cold** when it is delivered or rejected, and owes no
  reply (`expect_reply` false, or settled). Everything else is hot: queued,
  or owed.
- An agent is **cold** when archived. A restore makes it hot again.

### sb-core (`bend/hub/model.bend`, `core.bend`, `view.bend`)

1. `St` gains `old: Cold` next to `msgs` and `agents`. `Cold` keeps, per
   message, only what a later step can ask about: `id → (from, to,
   thread)` (a `reply_to` an old message names its thread and its asker),
   and per archived agent its whole `Agent` (a restore, `sb list
   --archived`, the names a new task may not reuse).
2. A message moves to `old` the step it turns cold (`message_state`
   delivered with no reply owed, `message_settled`); the replay does the
   same as it reads the journal, so a boot builds a small hot state.
3. The checks that walk `msgs` (owed replies, queued inputs, waiters)
   walk the hot list only; a lookup by id tries hot, then `old`.
4. `lifecycle archived` moves the agent to `old`; `restore` moves it back.
   Name checks (a new task's name, a rename) look in both.
5. `view.bend`: `msgs` and `agents` are the hot ones. `all_agents` (the
   TUI's /archived, `sb list --archived`) adds the cold agents, built once
   and only when asked.

Law (LAWS.bend): **`cold_is_inert`**: for any journal, a run with the
split
and a run with everything hot give the same effects for every input
(the same journal lines, the same says, the same view minus the cold
rows). PROOF.bend checks it on generated histories.

### Rust mirror (`core.rs`, `board.rs`, `model.rs`)

6. `State.msgs` holds the hot messages only (the view sends nothing
   else); `Waiting::of` and `unanswered_for` are then O(hot).
7. The archived agents' rows: built once when the view sends them
   (`all_agents`), kept until an agent's lifecycle changes, instead of
   each snapshot.

### What does not change

The journal, the transcripts, `sb inspect` and `sb history` (they read the
transcripts, not the state), the clients' protocol (the snapshot keeps
the same fields; archived rows still come when asked).

## Order (one sha each, the same proof each time)

1. **Measure** first, on a copy of his journal (bench_step, now that
   test_home keeps SB_BENCH_JOURNAL): a step's time split into sb-core and
   the Rust side, for a tick, a REPL line and a user line, and how it grows
   with the history (half the journal vs all of it). If sb-core's share is
   small, steps 3-4 wait and 2 goes first.
2. Rust mirror (6, 7): no Bend change.
3. sb-core messages (1-3, 5) with `cold_is_inert`.
4. sb-core archived agents (4, 5).

Each sha: before/after numbers from bench_step on the copy, and the
byte-equal check bcca8231 used (old and new code on the same journal
copy: the same snapshot JSON, the same agent contexts, the same rosters,
after a tick, a hello and a user line). Gates: quick, PROOF, proto_e2e,
tui_queue_tmux, tui_archived_tmux.

## Architect's conditions (m_15238)

1. **The wire stays as released.** The hub's `agents` state and the older
   state events keep sending the archived rows exactly as today (an older
   desktop core and the TUI's '▸ N archived' read them): they are only
   built once and cached until a lifecycle changes. The byte-equal check
   of each sha is the guard for this.
2. **sb-core shas**: PROOF (ALL PROOFS CHECK) with `cold_is_inert`;
   `old_core_replay` both ways (the journal is unchanged: the released
   core must replay what the new core wrote, and the new core what the
   released one wrote); a boot-time number on his journal copy, before and
   after (issue 15).
3. **The cold index keeps whatever a later op can name**: id → from, to,
   thread, plus expect_reply and settled if any path can still ask. The
   issue lists those paths and how each was checked, so 'nothing asks'
   is shown, not assumed. Paths to check: `sb wait <id>` on an old id, a
   late reply (`reply_to` an old message), a card that cites a message
   (`for_msg`), `sb inspect`/`sb history` (transcripts: no), the
   undelivered and queue marks of the clients, x-hub `x_ack`/`x_reply`.
4. **Measure, ship, re-measure.** If the measure says the Rust side
   dominates, step 2 ships alone and the history is measured again before
   sb-core is touched. Stop when a step's gain on his journal is below
   ~10 % of a tick.

## Risks

- A cold message that turns hot again: none today (a delivered message
  owing no reply has no further state); the law would catch a new one.
- A reply to a very old message: the cold index keeps its thread and
  asker, so routing is the same.
- Boot: the replay moves messages as it goes, so a boot ends with the
  small state (issue 15 gets faster too, not measured yet).
