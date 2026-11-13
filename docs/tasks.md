# Tasks (the work he asked for)

Spec: page tasks-center (v19/v20: the model, the status table, flows F1-F11, the failure cases,
step 6 "the Board in a panel tab"). Signed by architect: m_64423, m_64425.

## The word

A **task** in the UI, in the `sb task …` verbs and in what agents read is a **Work** in the code
(`S.Work`, `bend/hub/work.bend`, journal lines `work_*`, fields `works`). In the hub's code a
"task" already means an agent (`task_created`, `create_task`, `sb tasks`): do not rename either
side.

## Where it lives

- sb-core owns the works (`St.works`, folded ones in `St.tombs`, his artifact moves in `St.moved`),
  rebuilt from the journal: `work_created`, `work_changed {id, op, by, at}`, `work_folded`,
  `work_turn` (an agent's turn's works as they change: a new turn sets them, a steered delivery
  adds), `card_moved`, `art_moved`. `model.bend`'s apply is the only writer.
- Every message, card and timer carries its works (`works`, omitted when empty); an agent's turn
  carries `turn_works`. Whatever a turn makes inherits them; `--task <id>` replaces them; a notice
  from the hub never carries any.
- The status is computed in sb-core's view from the facts (cards, his page waits, its agents'
  runs and waits, `now`), never stored and never from an agent's word (`bend/hub/wstatus.bend`,
  first that holds: done, paused, needs you, working, ongoing, stalled by a crash, waiting on a
  reply, a timer or a watch, to review, then quiet: waiting, stalled after 2 h). A step's view
  carries rows (`works`) only for the works it touched: named by its events, one of their agents
  changed, his cards or page waits changed, or the 2 h crossed between the step's two clocks;
  `works_gone` and `tombs` for the folded ones. view_all's head carries them all.
- `sb task list [--all]` and `sb task show <id>` are the hub's reads of those rows
  (`rust/switchboard/src/task_list.rs`): words only, no decision.
- The shell stamps artifacts and pages at the door from the view's `turn_works` (as `turn_ms`) and
  writes a `Bise-Task: <id>` trailer per work on a land; it decides nothing.
