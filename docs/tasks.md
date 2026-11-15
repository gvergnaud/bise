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
- His words: a Task tab's carry its task; his plain words to an agent other than main carry its
  turn's work when it is exactly one; to main, none (main routes; its new task's agent is spawned
  with `--task <new id>`). An agent's `sb report blocked` is one card for him (`from_report`: his
  card whatever its kind, `user_card` in cardkind.bend and model.rs, rows in
  rust/proto/fixtures/user_card.jsonl). To review reads its agents' done without main's.
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
  writes a `Bise-Task: <id>` trailer per work on a land; it decides nothing. The land record
  (`lands.jsonl`) keeps the lander's works too (`works`), the Log's typed land fact.

## The Log (roma idea 3; architect m_65317, m_65437)

A task's Log is a read, never rows kept: `task/log` (the Task tab's Log) and `sb task show <id>
--log` read the journal from its end on a thread (`daemon/task_log.rs`, never the loop) until the
task's birth, and the land records; `switchboard/src/task_log.rs` makes the lines
(`rows::LogLine`): its agents' `sb report`s (the `reported` line's `report.works`, the reporter's
`--task` or turn's works), his answers (`card_closed` of a card whose works hold it), his gestures
(`work_changed` by `user`, `art_moved`/`card_moved` in and out), joins (an agent's first
`work_turn`), lands (`lands.jsonl` works). A merged task's facts read as the one it went into. Its
records are pinned by the journal codec's fixture (`core_journal.jsonl`). Worst case on a 116 MB
journal: 0.46 s.

Chunk 2b (tasks-ux m_65504, architect m_65507, m_65555), each from a fact stamped when it
happened, never rebuilt later:
- Question: a `card_opened` whose works hold it, his answer folded under it (one entry); a card
  moved onto the task after it was asked keeps its answer alone.
- Artifact: each version of an artifact or a page carries the works of the turn that made it
  (`Version.works`, stamped at the door with the Meta's union, never changed after; None before
  the stamp: the Meta's works). The shell reads the stores off the loop (`arts_of`); back-to-back
  versions of one artifact by one agent fold into one line ('v2→v5'), in the window and the CLI.
  His move stays its own gesture line.
- Break: a `lifecycle` failed/stopped line carries the turn's works it broke (`works`, absent when
  none: an older line gives no Break).
- Restart: its own op, WRestart (`sb task restart <id>`, the window's Restart): the work
  unchanged; its failed agents restored once through the restore path (one that fails again stays
  failed: the next Restart is his act again), each restored or idle one gets one hub note with
  works exactly [id] (law restart_note_carries_only_its_work), main aside; refused on a done,
  merged or paused task. Journaled `restart` by whoever made it: the Log says "main restarted it"
  for main's, never "you".

## Move to task… (F9; tasks-ux m_65409, designer m_65406, architect m_65565)

His gesture on an artifact (its native menu: the Library row's ⋯, its frame's ⋯, a right click:
Move to task…, Also add to task…, Remove from <task> with 2+ tasks) or a card (the Task tab's
Needs you ⋯): `tasks/moveTo.ts` says what the picker offers (open tasks, the ones he opened lately
first, then the Board's order; done lately at the end for an artifact; 'No task' last) and what a
pick makes of the works; `tasks/MoveHost.tsx` (mounted once in App.tsx) draws the picker where he
clicked and the 6 s toast. A pick sends `task_move` with exactly the new works (his for good:
moved_by_you_sticks); Undo sends the works before, exactly, a new gesture. Main moves on his word
with `sb task move <task-id>... (--art|--page|--card) [--none]`: main only (an agent is refused),
the same sb-core step, the line's `by` main (absent: him; laws a_move_by_main_says_main,
a_move_as_him_is_his_gesture_only); the Log says "main moved … here", never "you".

## A joining agent's context (roma idea 5; architect m_65317, m_65509)

An agent's delivery for a task it never served (it is not in `Work.agents`, read before the step's
`work_turn`) carries the task's context in front of its messages: Fx deliver's `ctx`, a hub part
the shell renders as `<task_context>…</task_context>`, never added to anyone's words. A brief that
carries the task is its first such delivery. The context is `bend/hub/wctx.bend`'s `ctx(ids, st,
pw)`, in the order a cut keeps (tasks-ux m_65542, architect m_65544): its name, his words that
started it (`Work.said`: on `sb task new` the shell reads his newest message to the maker, one
line, 300 chars, as `said`/`said_msg`; sb-core stamps it once at birth when `said_msg` is the
work's origin; an older work has none), why, pause, done-when ticked or not, what waits on him
(its open cards, one line each, and the count of its items on his pages, R.pw), its agents'
newest decisions, its agents with their `sb status` note, their newest reports; cut from the end
at 2000 chars, its last line `More: sb task show <id> --log` kept. No artifact list (sb-core keeps
none: the Log has them). Main gets none (it routes). Its words are the catalog's `core` area
(`rust/proto/src/words/core.rs`), generated into `bend/hub/gen/words.bend` (core_gen): sb-core's
inline English moves there one at a time. Laws: work_context_only_for_new_works,
work_context_for_a_joined_work, work_context_is_capped (bend/laws/work.bend).

## His gestures' Undo, the dot (roma ideas 2, 4)

Desktop only (`apps/desktop/src/tasks`): Undo sends the inverse gesture (undo.ts; Merge has none);
'new since you looked' compares each task with what he saw of it (seen.ts, his kept view, no hub
state).
