# 22 · an agent's row says `working` and `turns` from two sources

Status: agreed by architect (m_16053), not started. First item of
client-protocol's after-the-release list. Found by proto-zone-b
(m_16050, 2026-10-10) through tests/tui_queue_tmux.py, red 3 times in 7
runs on client-protocol after the feed switch (4733e3ba). Label: hub,
protocol.

## The problem

Since client-protocol step 4, the terminal takes an agent's turn edges
from the `hub/agents` row (bise_proto `rows::Agent`): its status
(`working`) and its count of ended turns (`turns`, 705aaaa4). The hub
fills the two halves from two sources, and sends the row again when
either moves:

- `status` comes from sb-core's state (the `state` the core computes
  when it applies a line);
- `turns` comes from the thread heads in `proto_view/heads.rs`, counted
  from the live `turn_done` lines; `emit.rs`'s line arm sends the agents
  rows again when a head moves.

So for a moment the two halves can disagree: one row says the turn ended
(`working` false) with the old count, the next one brings the count. A
reader that takes each row as the truth sees a second, made-up turn:
`(false, n)` ends the real turn, then `(false, n + 1)` looks like a turn
that started and ended between two rows.

What it broke: the queue guard (766a28a6). Two lines queued 50 ms apart:
the first went at the real end, then the late count drew a made-up
`[start, end]`, `queue::seen` cleared the mark and the made-up end sent
the second line at once, steered into the first one's turn.

## What client-protocol does about it (to delete when this is fixed)

The terminal keeps an owed end: one pure fn next to `rust/tui/src/queue.rs`'s
laws, `(previous row, new row, owed) -> edges` (proto-zone-b's queue-race
sha on client-protocol). An end drawn from the `working` flip is owed to
the count: when the count arrives late, that end is settled and nothing is
drawn. Its laws cover both orders (status first, count first) and a turn
with no entry. This is a reader papering over a writer: **delete that fn
and its owed state when this issue lands**, and let the edges come from
the row as it is.

## The fix

One row update carries both halves, from one source: when a line moves an
agent's turn, the hub sends the row once, with the status and the count of
the same step. Either `turns` is read from sb-core (the core already sees
every `turn_done` it applies), or the line arm's row waits for the core's
status of that same line. A law in switchboard: over a corpus of turns
(quick turns, a turn with no entry, an interrupted one, a failed one),
every agents row the hub sends has `working == false` iff its `turns`
counts the turn that just ended; no row ever has the new status with the
old count, or the old status with the new count.

Then: delete the owed-end fn (above), keep tests/tui_queue_tmux.py in the
gate, and check the window reads the edges the same way.
