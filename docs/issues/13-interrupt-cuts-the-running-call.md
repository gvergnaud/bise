# 13 · An interrupt should cut the running model call or tool, not wait for it

Status: open. Owner: the Bend runtime (bend/runtime, repl-live), assigned by main.

## What happens

Esc in the TUI, stop in the desktop app, or `sb interrupt` ends an agent's turn only when the call that
is running returns. qa-flows measured it on one hub (ambient-app 99da67ea, thread/interrupt check):

- the TUI: 7.3 to 7.9 s from esc to the end of the turn;
- the desktop app: 12.5 s.

The transcript says `obs: candidate_discarded: interrupt` then `obs: turn_done: interrupted`: the
model's answer is received in full, then thrown away. The hub side is immediate: sb-core emits
`fx interrupt` and the `stopped` line in the same step (515aa58d); the wait is in the runtime.

## Why it matters

The desktop bar asks for a stop under 1 s, and the TUI has the same wait. A stop that takes 8 to
12 s reads as "stop doesn't work", and the model call keeps costing tokens after the user said stop.

## Where to look

- the runtime's interrupt plan (`bend/runtime/repl-core-pure.bend`, `plan_interrupt` →
  `T.Interrupt`) and where the runtime checks for it between steps;
- the model request itself (the host's HTTP call): it must be cancellable, so an interrupt aborts the
  request in flight instead of letting it finish;
- a running tool: the bash tool's process group gets a TERM (then KILL after a short grace), the
  result says "interrupted by <who>" as the gate already does (`bend/runtime/gate-pure.bend`
  `interrupted`).

## Done when

- On the fake provider with a slow answer (a scripted delay of 20 s), an interrupt ends the turn in
  under 1 s, measured from the interrupt to `turn_done: interrupted`, in the TUI and in the desktop app.
- A running `sleep 30` in the bash tool is killed by the interrupt within 1 s, and its result says
  who interrupted.
- No regression: the existing interrupt tests (core_tests `an_interrupt_names_who_asked`, the TUI's
  `an_interrupt_mid_answer_is_a_dim_stop_naming_who`, `interrupt_says_stopped_once`) stay green; a
  turn that is not interrupted is unchanged; the session after an interrupt resumes normally on the
  next message (no half-written assistant message kept).
- Plan to architect before code (it is the Bend runtime).
