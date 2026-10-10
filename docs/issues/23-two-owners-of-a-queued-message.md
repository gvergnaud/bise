# 23 · two owners of a queued message: the terminal's queue and the hub's

Status: agreed by architect (m_16252), not started. Written with
queued_take (amb-feed). Label: hub, protocol, tui. Same area as issue 22.

## The problem

A message the user queues for a busy agent lives in one of two places,
depending on where he typed it:

- **the terminal** keeps its own queue, client-side
  (`rust/tui/src/queue.rs`: `App.queued`, tab queues, ↑ takes the newest
  back to edit). The hub never sees those lines until the terminal sends
  them, one per turn, when it decides the agent's turn ended (its
  `turn_edges` / `Owed` guard, 766a28a6);
- **the window** sends `send {mode: queued}`: sb-core holds the message
  (`Msg.queued`, law `user_queued_waits_for_turn_end`), the hub shows it on
  `rows::Agent.queued`, and `queued_take` takes it back
  (sb-core's `unqueue`, `Rejected{"taken_back"}`).

So "queued" has two owners. A line queued in the terminal is invisible to
the window (and to a second terminal), and the terminal's release depends
on its own reading of the turn's end, which issue 22 shows can be wrong
when the row's two halves disagree.

## The fix

The terminal moves to the hub-held queue:

1. tab sends `send {mode: queued}` at once, and the terminal draws its
   queue from `rows::Agent.queued` (the window's source);
2. ↑ (take the newest back to edit) calls `queued_take` and puts the
   returned `queued_taken.text` in the composer;
3. `queue.rs`'s client queue, `next_at`, `seen` and the `Owed` turn-edge
   guard go: sb-core's pump releases queued messages at the turn's end, in
   id order, once (`delivered_once`).

Then issue 22's made-up turn can't release a queued line any more (the
terminal no longer decides when), and both clients show the same queue.

## Proof when done

- tui_queue_tmux.py with the queue drawn from the hub's rows (tab, ↑ edit,
  order at the turn's end), and a second terminal seeing the same queue;
- proto_e2e's queued_take check unchanged;
- tui-parity's queue steps the same as before, or each difference named.
